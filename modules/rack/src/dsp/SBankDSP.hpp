// SPDX-License-Identifier: MIT OR Apache-2.0
//
// ============================================================================
// S-Bank DSP — header-only analog-emulation DSP for the S-Bank module family.
// ============================================================================
//
// What this is
// ------------
// A single-header, std-only, C++11 DSP library. It has **no VCV Rack SDK
// dependency** and compiles/tests headless. It provides two analog-emulation
// "voices" plus the supporting primitives:
//   * sbank::VactrolLpg  — Buchla-292-style vactrol low-pass gate (a
//                          state-space 292 cell driven by a vactrol model).
//   * sbank::StrikeCore  — a clean, zero-bleed, envelope-driven low-pass gate
//                          ("Strike") with an optional analogue-dirt layer.
//
// Canonical include
// -----------------
// The stable include path is:
//     #include <sbank/sbank_dsp.hpp>
// which forwards to this file. To vendor the library, copy this header (and
// optionally include/sbank/sbank_dsp.hpp) into your tree and add its directory
// to your compiler's include path (-I). Nothing else is required.
//
// Real-time-safety contract
// -------------------------
// Every public processing method is real-time safe: **no heap allocation, no
// locking, no exceptions, and no unbounded loops on the audio path**. All state
// is fixed-size and stack/inline. Setters that touch only POD fields are also
// RT-safe. Non-finite inputs are scrubbed to 0 at each public processSample so
// a NaN/Inf can never propagate into the recursive state.
//
// Signal conventions
// ------------------
//   * Audio is nominal +-1.0 full scale (host wrappers scale +-5 V <-> +-1).
//   * CV is in volts. The vactrol gate opens for roughly +7..+11 V of CV.
//   * Strike's ctrl01 is in [0,1]; its hitV is gate-volts and is edge-triggered
//     (a rising edge past kHitThresholdV fires the percussion envelope).
//
// Public surface
// --------------
//   Voices:      VactrolLpg, StrikeCore
//   Voice config:LpgParams, LpgComponents, LpgMode, Oversample,
//                StrikeParams, Material, StrikeImperfectionConfig
//   Primitives:  Envelope, Gate, PitchTracker, PinkNoise, XorShift, SplitMix64
//   Internals:   everything in namespace sbank::detail (Cell292, ControlPath,
//                AudioPath, VactrolModel, ControlCoeffs, HalfbandStage,
//                Oversampler, halfband-tap helpers, R3/halfband constants).
//
// Quick start (per voice)
// ----------------------
//   sbank::VactrolLpg lpg;            // construct
//   lpg.setSampleRate(48000.f);       // tell it the host SR
//   lpg.setParams(sbank::LpgParams{});// configure (or use a positional shim)
//   for (...) out = lpg.processSample(audioIn, cvIn);   // run per sample
//
//   sbank::StrikeCore st;
//   st.setSampleRate(48000.f);
//   st.setParams(sbank::StrikeParams{});
//   for (...) out = st.processSample(audioIn, ctrl01, decayMod, hitV);
//
// ============================================================================
#pragma once

#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstddef>
#include <limits>

// Library version. Bump on releases; vendored copies can branch on these.
#define SBANK_DSP_VERSION_MAJOR 1
#define SBANK_DSP_VERSION_MINOR 0
#define SBANK_DSP_VERSION_PATCH 0
#define SBANK_DSP_VERSION \
    (SBANK_DSP_VERSION_MAJOR * 10000 + SBANK_DSP_VERSION_MINOR * 100 + SBANK_DSP_VERSION_PATCH)

namespace sbank {

// The closed-form solves assume IEEE-754 single precision (denormal/Inf/NaN
// semantics, exact divider identities). Guard against exotic float formats.
static_assert(std::numeric_limits<float>::is_iec559, "sbank DSP assumes IEEE-754 float");

// ===== Shared constants & helpers ==========================================

constexpr float kPi = 3.14159265358979323846f;

// Rising-edge threshold (in gate volts) above which a Strike HIT fires the
// percussion envelope. Strike's hitV input is compared against this.
constexpr float kHitThresholdV = 0.25f;

inline float clampf(float v, float lo, float hi) {
    return std::min(std::max(v, lo), hi);
}

// Flush a subnormal magnitude to exactly zero. Applied to persisted recursive
// IIR state so denormals can't stall the FPU. The flushed values are < 1e-30,
// far below any audible / golden-tolerance level.
static inline float flushDenorm(float x) {
    return std::fabs(x) < 1.0e-30f ? 0.f : x;
}

// ===== Public voice configuration ==========================================

// VactrolLpg processing topology.
//   Both    — VCA + low-pass (the full 292 cell, C3 in circuit).
//   Vca     — amplitude only (C3 switched out, R3 -> VCA value).
//   Lowpass — filter character emphasised (full cell).
enum class LpgMode {
    Both = 0,
    Vca = 1,
    Lowpass = 2,
};

// Oversampling factor for the audio path. The factor snaps to 1, 2, or 4.
enum class Oversample {
    x1 = 1,
    x2 = 2,
    x4 = 4,
};

// Tunable parameters of the vactrol low-pass gate.
struct LpgParams {
    LpgMode mode;        // processing topology (see LpgMode)
    float resonance;     // [0,1] resonance / feedback amount (clamped in cell)
    float cvOffset;      // volts added to the CV before the control path
    float drive;         // >=0 input drive into the tanh nonlinearity (1 = unity)
    uint32_t oversample; // requested factor; oversampleFactor() snaps to 1/2/4

    LpgParams()
        : mode(LpgMode::Both), resonance(0.2f), cvOffset(0.f), drive(1.f), oversample(2) {
    }

    // Resolve the requested oversample request to the supported 1/2/4 set.
    size_t oversampleFactor() const {
        if (oversample <= 1)
            return 1;
        if (oversample <= 3)
            return 2;
        return 4;
    }
};

// Physical component values of the modelled vactrol + 292 cell. Defaults are
// the datasheet/reference values; override via VactrolLpg::setComponents to
// change the analog character. Units annotated per field.
struct LpgComponents {
    float c1;        // farads  — first cell capacitor
    float c2;        // farads  — second cell capacitor
    float c3;        // farads  — resonance/feedback capacitor (0 in VCA mode)
    float rfLawA;    // ohms    — power-law numerator: Rf = A / If^1.4 + B
    float rfLawB;    // ohms    — power-law offset (asymptotic on-resistance)
    float rOnMin;    // ohms    — minimum (fully open) vactrol resistance
    float rOff;      // ohms    — maximum (fully closed) vactrol resistance
    float tauAttack; // seconds — vactrol opening (attack) time constant
    float tauDecay;  // seconds — vactrol closing (decay) time constant

    LpgComponents()
        : c1(1.0e-9f),
          c2(220.0e-12f),
          c3(4.7e-9f),
          rfLawA(3.464591f),
          rfLawB(1136.213f),
          rOnMin(500.f),
          rOff(10.0e6f),
          tauAttack(0.005f),
          tauDecay(0.120f) {
    }
};

// ===== Reusable primitives =================================================
// (Envelope, Gate, PitchTracker, PinkNoise, XorShift, SplitMix64 — genuinely
//  reusable building blocks, defined further below alongside the Strike voice.)

namespace detail {

// ----- Internal constants ---------------------------------------------------

// R3 in the 292 cell: a small value in VCA mode (C3 switched out) and a large
// value in filter modes.
constexpr float kR3Vca = 1.0e5f;
constexpr float kR3Filter = 1.0e6f;

// Polyphase halfband FIR lengths for the 2x interpolation stages.
constexpr size_t kHalfbandLen = 61;
constexpr size_t kHalfbandEvenLen = 31;
constexpr size_t kHalfbandOddLen = 30;

// ----- ControlCoeffs --------------------------------------------------------
// Precomputed coefficients of the control-path LED-current curve fit. The
// offset/scale controls position the wiper (offsetControl) and the variable
// resistance (scaleControl); the golden defaults are 0.0 / 1.0.
class ControlCoeffs {
  public:
    float ifMin; // amps — minimum LED forward current
    float ifMax; // amps — maximum LED forward current

    // offsetControl in [0,1]: wiper position of the control pot (0 default).
    // scaleControl  in [0,1]: fraction of the variable resistor (1 default).
    explicit ControlCoeffs(double offsetControl = 0.0, double scaleControl = 1.0) {
        const double ifmin = 10.1e-6;
        const double ifmax = 40e-3;
        const double r2max = 10e3;
        const double r6max = 20e3;
        const double r7 = 33e3;
        const double r3 = 150e3;
        const double r5 = 100e3;
        const double r8 = 4.7e3;
        const double r9 = 470.0;
        const double vbConst = 3.9;
        const double vt = 26e-3;
        const double n = 3.9696;
        const double kl = 6.3862;
        const double g = 2e5;
        const double vs = 15.0;
        const double gamma = 0.0001;

        const double offset = 0.9999 * clampd(offsetControl, 0.0, 1.0) + 0.0001;
        const double scale = clampd(scaleControl, 0.0, 1.0);
        const double r6 = scale * r6max;
        const double r1 = (1.0 - offset) * r2max;
        const double r2 = offset * r2max;
        const double r6r7d = r6 + r7;

        const double alphaD = 1.0 + r6r7d * (1.0 / r3 + 1.0 / r5);
        const double betaD = ((1.0 / alphaD) - 1.0) / r6r7d - 1.0 / r8;
        const double bound1D = 600.0 * alphaD * n * vt / (g * (r6r7d - 1.0 / (alphaD * betaD)));
        const double biasD = vs / (r3 * (1.0 + r1 / r2));
        const double xCoeffD = g * (r6r7d - 1.0 / (alphaD * betaD)) / (alphaD * n * vt);
        const double v3WD = -(alphaD / g) * n * vt;
        const double v3IaD = -1.0 / (alphaD * betaD);
        const double v3SatConstD = kl * alphaD / g * n * vt;
        const double invAlphaD = 1.0 / alphaD;
        const double ifBound2D = vbConst / r6r7d;
        const double ifBound3D =
            (gamma * g * vbConst + alphaD * r9 * (vbConst * betaD + ifmax)) /
            (gamma * g * r6r7d + r9);
        const double ifb3SlopeD = gamma * g * r6r7d / (alphaD * r9) + invAlphaD;
        const double ifb3ConstD = -gamma * g * vbConst / (alphaD * r9) - vbConst * betaD;

        ifMin = static_cast<float>(ifmin);
        ifMax = static_cast<float>(ifmax);
        bias = static_cast<float>(biasD);
        invR5 = static_cast<float>(1.0 / r5);
        bound1 = static_cast<float>(bound1D);
        xCoeff = static_cast<float>(xCoeffD);
        v3W = static_cast<float>(v3WD);
        v3Ia = static_cast<float>(v3IaD);
        v3SatConst = static_cast<float>(v3SatConstD);
        r6r7 = static_cast<float>(r6r7d);
        alpha = static_cast<float>(alphaD);
        beta = static_cast<float>(betaD);
        invAlpha = static_cast<float>(invAlphaD);
        ifBound2 = static_cast<float>(ifBound2D);
        ifBound3 = static_cast<float>(ifBound3D);
        ifb3Slope = static_cast<float>(ifb3SlopeD);
        ifb3Const = static_cast<float>(ifb3ConstD);
    }

    // Map a (smoothed) control voltage vb to LED forward current, in amps.
    float current(float vb) const {
        const float cv = clampf(vb, -10.f, 50.f);
        const float ia = cv * invR5 + bias;

        float v3 = 0.f;
        if (ia <= -bound1) {
            v3 = v3Ia * ia;
        }
        else if (ia < bound1) {
            const float x = xCoeff * ia;
            const float w = k0 + x * (k1 + x * (k2 + x * k3));
            v3 = v3W * w + v3Ia * ia;
        }
        else {
            v3 = v3SatConst - ia * r6r7;
        }

        const float ifBound1 = alpha * (ifMin - beta * v3);
        float out = ifMin;
        if (ia <= ifBound1) {
            out = ifMin;
        }
        else if (ia <= ifBound2) {
            out = beta * v3 + ia * invAlpha;
        }
        else if (ia <= ifBound3) {
            out = ifb3Slope * ia + ifb3Const;
        }
        else {
            out = ifMax;
        }
        return clampf(out, ifMin, ifMax);
    }

  private:
    static double clampd(double v, double lo, double hi) {
        return std::min(std::max(v, lo), hi);
    }

    float bias;
    float invR5;
    float bound1;
    float xCoeff;
    float v3W;
    float v3Ia;
    float v3SatConst;
    float r6r7;
    float alpha;
    float beta;
    float invAlpha;
    float ifBound2;
    float ifBound3;
    float ifb3Slope;
    float ifb3Const;
    static constexpr float k0 = 146.8f;
    static constexpr float k1 = 0.49202f;
    static constexpr float k2 = 4.1667e-4f;
    static constexpr float k3 = 7.3915e-9f;
};

// ----- ControlPath ----------------------------------------------------------
// Smooths the incoming CV (one-pole) and maps it to LED current via a
// configurable ControlCoeffs (defaults preserve the golden).
class ControlPath {
  public:
    explicit ControlPath(float sampleRate = 48000.f) : cvState(0.f), smooth(0.f), coeffs() {
        setSampleRate(sampleRate);
    }

    void setSampleRate(float sampleRate) {
        smooth = std::exp(-1.f / (0.0015f * std::max(sampleRate, 1.f)));
    }

    // Replace the curve-fit coefficients (e.g. to move the control pot).
    void setCoeffs(const ControlCoeffs& c) {
        coeffs = c;
    }
    const ControlCoeffs& getCoeffs() const {
        return coeffs;
    }

    void reset() {
        cvState = 0.f;
    }

    // cv: control voltage; offset: volts added before smoothing. Returns amps.
    float process(float cv, float offset) {
        const float target = cv + offset;
        cvState = target + (cvState - target) * smooth;
        return coeffs.current(cvState);
    }

  private:
    float cvState;
    float smooth;
    ControlCoeffs coeffs;
};

// ----- VactrolModel ---------------------------------------------------------
// Datasheet power-law target resistance plus an asymmetric, state-dependent
// one-pole (fast attack, slow decay). Owns its LpgComponents.
class VactrolModel {
  public:
    explicit VactrolModel(float sampleRate = 48000.f)
        : sr(sampleRate), comp(), rf(LpgComponents().rOff) {
    }

    void setSampleRate(float sampleRate) {
        sr = std::max(sampleRate, 1000.f);
    }

    float getSampleRate() const {
        return sr;
    }

    void setComponents(const LpgComponents& c) {
        comp = c;
    }
    const LpgComponents& getComponents() const {
        return comp;
    }

    void reset() {
        rf = comp.rOff;
    }

    float resistance() const {
        return rf;
    }

    // ifCurrent: LED forward current (amps). Returns vactrol resistance (ohms).
    float process(float ifCurrent) {
        const float iEff = std::max(ifCurrent, 1.0e-7f);
        const float target =
            clampf(comp.rfLawA / std::pow(iEff, 1.4f) + comp.rfLawB, comp.rOnMin, comp.rOff);

        const bool opening = target < rf;
        float tau = opening ? comp.tauAttack : comp.tauDecay;
        const float span = std::log(comp.rOff / comp.rOnMin);
        const float openness = clampf(std::log(comp.rOff / rf) / span, 0.f, 1.f);
        tau *= 0.5f + 0.5f * (1.f - openness);

        const float alpha = std::exp(-1.f / (tau * sr));
        rf = target + (rf - target) * alpha;
        return rf;
    }

  private:
    float sr;
    LpgComponents comp;
    float rf;
};

// ----- Halfband taps --------------------------------------------------------
inline std::array<float, kHalfbandLen> makeHalfbandTaps() {
    std::array<float, kHalfbandLen> h;
    const size_t c = (kHalfbandLen - 1) / 2;
    float sum = 0.f;
    for (size_t k = 0; k < kHalfbandLen; ++k) {
        const float x = 0.5f * (static_cast<float>(k) - static_cast<float>(c));
        const float sinc = (x == 0.f) ? 1.f : std::sin(kPi * x) / (kPi * x);
        const float n = static_cast<float>(kHalfbandLen - 1);
        const float w = 0.42f - 0.5f * std::cos(2.f * kPi * static_cast<float>(k) / n) +
                        0.08f * std::cos(4.f * kPi * static_cast<float>(k) / n);
        h[k] = 0.5f * sinc * w;
        sum += h[k];
    }
    for (size_t k = 0; k < kHalfbandLen; ++k)
        h[k] /= sum;
    return h;
}

inline const std::array<float, kHalfbandLen>& halfbandTaps() {
    static const std::array<float, kHalfbandLen> taps = makeHalfbandTaps();
    return taps;
}

// ----- HalfbandStage --------------------------------------------------------
// One 2x polyphase interpolation/decimation stage wrapping a per-subsample fn.
class HalfbandStage {
  public:
    HalfbandStage() {
        const std::array<float, kHalfbandLen>& taps = halfbandTaps();
        h = taps;
        for (size_t i = 0; i < kHalfbandEvenLen; ++i)
            he[i] = taps[2 * i];
        for (size_t i = 0; i < kHalfbandOddLen; ++i)
            ho[i] = taps[2 * i + 1];
        reset();
    }

    void reset() {
        xh.fill(0.f);
        y2.fill(0.f);
    }

    template <typename F>
    float process(float x, F& fn) {
        push(xh, x);

        float up0 = 0.f;
        for (size_t i = 0; i < kHalfbandEvenLen; ++i)
            up0 += he[i] * xh[i];

        float up1 = 0.f;
        for (size_t i = 0; i < kHalfbandOddLen; ++i)
            up1 += ho[i] * xh[i];

        up0 *= 2.f;
        up1 *= 2.f;

        push(y2, fn(up0));
        push(y2, fn(up1));

        float out = 0.f;
        for (size_t i = 0; i < kHalfbandLen; ++i)
            out += h[i] * y2[i];
        return out;
    }

  private:
    template <size_t N>
    static void push(std::array<float, N>& hist, float x) {
        for (size_t i = N - 1; i > 0; --i)
            hist[i] = hist[i - 1];
        hist[0] = x;
    }

    std::array<float, kHalfbandLen> h;
    std::array<float, kHalfbandEvenLen> he;
    std::array<float, kHalfbandOddLen> ho;
    std::array<float, kHalfbandEvenLen> xh;
    std::array<float, kHalfbandLen> y2;
};

// ----- Oversampler ----------------------------------------------------------
// 1x/2x/4x oversampling of an arbitrary per-sample function via cascaded
// halfband stages.
class Oversampler {
  public:
    void reset() {
        stageA.reset();
        stageB.reset();
    }

    template <typename F>
    float process(float x, size_t factor, F& fn) {
        if (factor <= 1)
            return fn(x);
        if (factor == 2)
            return stageA.process(x, fn);

        auto inner = [this, &fn](float s) -> float {
            return stageB.process(s, fn);
        };
        return stageA.process(x, inner);
    }

  private:
    HalfbandStage stageA;
    HalfbandStage stageB;
};

// ----- Cell292 --------------------------------------------------------------
// Parker & D'Angelo 3-capacitor state-space 292 cell, solved as a stable
// delay-free loop. Component/coefficient fields are set via guarded setters
// (caps/Rf clamped strictly positive); the recursive state is private.
class Cell292 {
  public:
    Cell292()
        : f_(0.5f / 48000.f),
          rf_(1.0e6f),
          r3_(kR3Filter),
          c1_(1.0e-9f),
          c2_(220.0e-12f),
          c3_(4.7e-9f),
          resonance_(0.f),
          drive_(1.f),
          sx(0.f),
          so(0.f),
          sd(0.f),
          xo(0.f) {
    }

    // Guarded setters. Capacitances and Rf are clamped to a tiny positive
    // epsilon so a zero/negative value can't divide-by-zero in the solve.
    void setTimestep(float f) {
        f_ = f;
    }
    void setRf(float rf) {
        rf_ = std::max(rf, 1.f);
    }
    void setR3(float r3) {
        r3_ = std::max(r3, kEps);
    }
    void setC1(float c1) {
        c1_ = std::max(c1, kEps);
    }
    void setC2(float c2) {
        c2_ = std::max(c2, kEps);
    }
    void setC3(float c3) {
        c3_ = std::max(c3, 0.f); // C3 may legitimately be 0 (VCA mode)
    }
    void setResonance(float r) {
        resonance_ = r;
    }
    void setDrive(float d) {
        drive_ = d;
    }

    void reset() {
        sx = 0.f;
        so = 0.f;
        sd = 0.f;
        xo = 0.f;
    }

    // True if the recursive state is all finite.
    bool isFinite() const {
        return std::isfinite(sx) && std::isfinite(so) && std::isfinite(sd) && std::isfinite(xo);
    }

    float solveStep(float x) {
        const float a1 = 1.f / (c1_ * rf_);
        const float a2 = -(1.f / rf_ + 1.f / r3_) / c1_;
        const float b1 = 1.f / (rf_ * c2_);
        const float b2 = -2.f / (rf_ * c2_);
        const float b3 = 1.f / (rf_ * c2_);
        const float b4 = c3_ / c2_;
        const float d2 = -1.f;

        float d1 = 0.f;
        if (c3_ > 0.f) {
            const float amax = (2.f * c1_ * r3_ + (c2_ + c3_) * (r3_ + rf_)) / (c3_ * r3_);
            d1 = clampf(resonance_, 0.f, 1.f) * amax;
        }

        float gx = xo;
        float s2 = 1.f;
        if (drive_ > 0.f) {
            const float t = std::tanh(drive_ * xo);
            gx = t / drive_;
            s2 = 1.f - t * t;
        }

        const float dx = 1.f / (1.f - b2 * f_);
        const float do_ = 1.f / (1.f - a2 * f_);
        const float dmas =
            1.f / (1.f - dx * (f_ * f_ * b3 * do_ * a1 + b4 * f_ * d1 * s2 * do_ * a1 + b4 * d2));
        const float nl = d1 * (gx - xo * s2);

        const float yx =
            (sx + f_ * b1 * x + f_ * b3 * do_ * so + f_ * b4 * (sd + (1.f / f_) * nl) +
             b4 * d1 * s2 * do_ * so) *
            dx * dmas;
        const float yo = (so + f_ * a1 * yx) * do_;
        const float yd = (sd + (1.f / f_) * nl) + (1.f / f_) * (d1 * s2 * yo + d2 * yx);

        sx += 2.f * f_ * (b1 * x + b2 * yx + b3 * yo + b4 * yd);
        so += 2.f * f_ * (a1 * yx + a2 * yo);
        if (c3_ > 0.f) {
            sd = -(sd + (2.f / f_) * nl) - (2.f / f_) * (d1 * s2 * yo + d2 * yx);
        }
        else {
            sd = 0.f;
        }
        xo = yo;

        // Flush persisted recursive state to avoid denormal stalls. Flushed
        // values are subnormal (<1e-30), far below the golden tolerance.
        sx = flushDenorm(sx);
        so = flushDenorm(so);
        sd = flushDenorm(sd);
        xo = flushDenorm(xo);
        return yo;
    }

  private:
    static constexpr float kEps = 1.0e-30f;

    float f_;
    float rf_;
    float r3_;
    float c1_;
    float c2_;
    float c3_;
    float resonance_;
    float drive_;

    float sx;
    float so;
    float sd;
    float xo;
};

// ----- AudioPath ------------------------------------------------------------
// Owns the 292 cell + oversampler plus its LpgParams/LpgComponents view, so
// process() takes only the audio sample and the current vactrol resistance.
class AudioPath {
  public:
    explicit AudioPath(float sampleRate = 48000.f) : sr(sampleRate), params(), comp() {
    }

    void setSampleRate(float sampleRate) {
        sr = std::max(sampleRate, 1000.f);
    }

    float getSampleRate() const {
        return sr;
    }

    void setParams(const LpgParams& p) {
        params = p;
    }
    void setComponents(const LpgComponents& c) {
        comp = c;
    }

    void reset() {
        cell.reset();
        oversampler.reset();
    }

    bool isFinite() const {
        return cell.isFinite();
    }

    // x: audio in (+-1 nominal); rf: vactrol resistance (ohms). Returns audio.
    float process(float x, float rf) {
        cell.setRf(rf);
        cell.setC1(comp.c1);
        cell.setC2(comp.c2);
        if (params.mode == LpgMode::Vca) {
            cell.setC3(0.f);
            cell.setR3(kR3Vca);
        }
        else {
            cell.setC3(comp.c3);
            cell.setR3(kR3Filter);
        }
        cell.setResonance(params.resonance);
        cell.setDrive(params.drive);

        const size_t factor = params.oversampleFactor();
        cell.setTimestep(0.5f / (sr * static_cast<float>(factor)));
        if (factor == 1)
            return cell.solveStep(x);

        auto solver = [this](float xs) -> float {
            return cell.solveStep(xs);
        };
        return oversampler.process(x, factor, solver);
    }

  private:
    float sr;
    LpgParams params;
    LpgComponents comp;
    Cell292 cell;
    Oversampler oversampler;
};

} // namespace detail

// ===== Public voice: VactrolLpg ============================================
//
// A Buchla-292-style vactrol low-pass gate. CV drives an LED-current control
// path, the vactrol model turns that into a resistance, and the state-space
// 292 cell filters/attenuates the audio. Construct -> setSampleRate ->
// setParams -> loop processSample.
class VactrolLpg {
  public:
    explicit VactrolLpg(float sampleRate = 48000.f)
        : sr(sampleRate),
          params(),
          comp(),
          control(sampleRate),
          vactrol(sampleRate),
          audio(sampleRate),
          lastRf(comp.rOff) {
        vactrol.setComponents(comp);
        audio.setComponents(comp);
        audio.setParams(params);
    }

    // sampleRate: host sample rate in Hz (clamped to a 1 kHz audio minimum).
    void setSampleRate(float sampleRate) {
        sr = std::max(sampleRate, 1000.f);
        control.setSampleRate(sr);
        vactrol.setSampleRate(sr);
        audio.setSampleRate(sr);
    }

    float getSampleRate() const {
        return sr;
    }

    // Typed parameter setter (preferred). Resonance/drive/cvOffset are clamped
    // to sane ranges at the boundary.
    void setParams(const LpgParams& p) {
        params = p;
        params.resonance = clampf(params.resonance, 0.f, 1.f);
        params.drive = clampf(params.drive, 0.f, 64.f);
        params.cvOffset = clampf(params.cvOffset, -100.f, 100.f);
        params.oversample = std::min<uint32_t>(params.oversample, 4);
        audio.setParams(params);
    }

    // Enum-typed overload. mode/resonance/cvOffset/drive as in LpgParams;
    // oversampleFactor snaps to 1/2/4.
    void setParams(LpgMode mode, float resonance, float cvOffset, float drive,
                   uint32_t oversampleFactor) {
        LpgParams p;
        p.mode = mode;
        p.resonance = resonance;
        p.cvOffset = cvOffset;
        p.drive = drive;
        p.oversample = oversampleFactor;
        setParams(p);
    }

    // Positional shim — kept for existing call sites. mode: 0=Both,1=Vca,
    // 2=Lowpass. Forwards into the enum/struct path.
    void setParams(uint32_t mode, float resonance, float cvOffset, float drive,
                   uint32_t oversample) {
        const LpgMode m =
            mode == 1 ? LpgMode::Vca : mode == 2 ? LpgMode::Lowpass : LpgMode::Both;
        setParams(m, resonance, cvOffset, drive, oversample);
    }

    const LpgParams& getParams() const {
        return params;
    }

    // Override the modelled component values (analog character). Stored so
    // reset() re-reads them; defaults preserve the golden sound.
    void setComponents(const LpgComponents& c) {
        comp = c;
        vactrol.setComponents(comp);
        audio.setComponents(comp);
    }
    const LpgComponents& getComponents() const {
        return comp;
    }

    void reset() {
        control.reset();
        vactrol.setComponents(comp);
        vactrol.reset();
        audio.setComponents(comp);
        audio.reset();
        lastRf = comp.rOff;
    }

    // audioIn: +-1 nominal audio; cvIn: control voltage. Non-finite inputs are
    // scrubbed to 0. Returns the filtered/gated audio sample.
    float processSample(float audioIn, float cvIn) {
        if (!std::isfinite(audioIn))
            audioIn = 0.f;
        if (!std::isfinite(cvIn))
            cvIn = 0.f;
        const float current = control.process(cvIn, params.cvOffset);
        const float rf = vactrol.process(current);
        lastRf = rf;
        return audio.process(audioIn, rf);
    }

    // Block helper: a trivially-correct loop over processSample.
    // in/out: length-n audio buffers; cv: length-n control-voltage buffer.
    void processBlock(const float* in, const float* cv, float* out, int n) {
        for (int i = 0; i < n; ++i)
            out[i] = processSample(in[i], cv[i]);
    }

    // Meter read-back: the last vactrol resistance (ohms). lastResistance() is
    // the canonical name; getMeter() is the common cross-voice accessor.
    float lastResistance() const {
        return lastRf;
    }
    float getMeter() const {
        return lastRf;
    }

    // True if all recursive state is finite.
    bool isFinite() const {
        return std::isfinite(lastRf) && audio.isFinite();
    }

    // Reset only the recursive state (leaves params/components intact).
    void scrubState() {
        control.reset();
        vactrol.reset();
        audio.reset();
        lastRf = comp.rOff;
    }

  private:
    float sr;
    LpgParams params;
    LpgComponents comp;
    detail::ControlPath control;
    detail::VactrolModel vactrol;
    detail::AudioPath audio;
    float lastRf;
};

// ===== Reusable primitives =================================================

// Voss-McCartney-style pink-noise shaper (filters supplied white noise).
class PinkNoise {
  public:
    PinkNoise() {
        reset();
    }

    void reset() {
        b.fill(0.f);
    }

    // white: a white-noise sample in [-1,1]. Returns a pink-noise sample.
    float next(float white) {
        b[0] = 0.99886f * b[0] + white * 0.0555179f;
        b[1] = 0.99332f * b[1] + white * 0.0750759f;
        b[2] = 0.96900f * b[2] + white * 0.153852f;
        b[3] = 0.86650f * b[3] + white * 0.3104856f;
        b[4] = 0.55000f * b[4] + white * 0.5329522f;
        b[5] = -0.7616f * b[5] - white * 0.016898f;
        const float pink = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + white * 0.5362f;
        b[6] = white * 0.115926f;
        return pink * 0.11f;
    }

  private:
    std::array<float, 7> b;
};

// Fast xorshift PRNG (32-bit output).
class XorShift {
  public:
    explicit XorShift(uint64_t seed = 0) {
        setSeed(seed);
    }

    void setSeed(uint64_t seed) {
        state = (seed ^ 0x9E3779B97F4A7C15ull) | 1ull;
    }

    uint32_t nextU32() {
        uint64_t x = state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        state = x;
        return static_cast<uint32_t>((x * 0x2545F4914F6CDD1Dull) >> 32);
    }

    // Returns a float in [-1, 1].
    float nextF32() {
        return (static_cast<float>(nextU32()) / 4294967295.0f) * 2.f - 1.f;
    }

  private:
    uint64_t state;
};

// SplitMix64 PRNG — used to seed tolerances deterministically.
class SplitMix64 {
  public:
    explicit SplitMix64(uint64_t seed) : state(seed) {
    }

    uint64_t next() {
        uint64_t z = (state += 0x9E3779B97F4A7C15ull);
        z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ull;
        z = (z ^ (z >> 27)) * 0x94D049BB133111EBull;
        return z ^ (z >> 31);
    }

    // Returns a float in [0, 1).
    float next01() {
        return static_cast<float>((next() >> 40) & 0xFFFFFFull) / 16777216.0f;
    }

  private:
    uint64_t state;
};

// ===== Public voice configuration (Strike) =================================

// Tunable parameters of the Strike voice.
struct StrikeParams {
    float open;     // [0,1] gate floor (minimum openness)
    float decay;    // [0,1] envelope decay time (short -> long)
    float material; // [0,1] timbre (hard -> soft)

    StrikeParams() : open(0.f), decay(0.4f), material(0.f) {
    }
};

// Maps Strike's 0..1 "material" knob to attack/cutoff/level endpoints. The
// from01 direction is hard (m=0) -> soft (m=1): higher m = slower attack, lower
// cutoff ceiling, lower level. Endpoints come from Material::Config.
struct Material {
    float attackTau;     // seconds — envelope attack time constant
    float cutoffCeiling; // Hz      — gate cutoff at full openness
    float level;         // linear  — output level scale

    // Character endpoints; defaults are the tuned literals (golden-preserving).
    struct Config {
        float attackTauBase;  // s  — attack tau at m=0 (hard)
        float attackTauRatio; // -- — attack tau multiplier from m=0 to m=1
        float cutoffBase;     // Hz — cutoff ceiling at m=0 (hard)
        float cutoffRatio;    // -- — cutoff multiplier from m=0 to m=1
        float levelBase;      // -- — output level at m=0
        float levelSlope;     // -- — output-level reduction across m

        Config()
            : attackTauBase(0.0005f),
              attackTauRatio(25.0f / 0.5f),
              cutoffBase(18000.f),
              cutoffRatio(2500.f / 18000.f),
              levelBase(1.f),
              levelSlope(0.4f) {
        }
    };

    static Material from01(float material, const Config& cfg = Config()) {
        const float m = clampf(material, 0.f, 1.f);
        Material out;
        out.attackTau = cfg.attackTauBase * std::pow(cfg.attackTauRatio, m);
        out.cutoffCeiling = cfg.cutoffBase * std::pow(cfg.cutoffRatio, m);
        out.level = cfg.levelBase - cfg.levelSlope * m;
        return out;
    }
};

// ===== Reusable primitives (Strike building blocks) ========================

// Dual-rate percussive envelope: a fast + slow exponential injected by a HIT
// edge, summed with a sustain control, slewed by an attack, and floored by an
// "open" level.
class Envelope {
  public:
    // Decay-range endpoints (slow-tail tau, in seconds, across decay01 0->1).
    struct Config {
        float fastTau;  // s — fast-tail time constant
        float slowMin;  // s — slow-tail tau at decay01=0
        float slowMax;  // s — slow-tail tau at decay01=1
        float hitBump;  // -- amount added to each tail on a HIT edge

        Config() : fastTau(0.012f), slowMin(0.030f), slowMax(3.5f), hitBump(0.35f) {
        }
    };

    explicit Envelope(float sampleRate = 48000.f)
        : sr(sampleRate), cfg(), eFast(0.f), eSlow(0.f), env(0.f), prevHitHigh(false) {
    }

    void setSampleRate(float sampleRate) {
        sr = std::max(sampleRate, 1000.f);
    }

    float getSampleRate() const {
        return sr;
    }

    void setConfig(const Config& c) {
        cfg = c;
    }
    const Config& getConfig() const {
        return cfg;
    }

    void reset() {
        eFast = 0.f;
        eSlow = 0.f;
        env = 0.f;
        prevHitHigh = false;
    }

    // decay01 [0,1] decay time; pitchFactor scales the slow tail; attackTau (s)
    // attack slew; ctrl01 [0,1] sustain; open01 [0,1] gate floor; hitV gate
    // volts (rising edge past kHitThresholdV fires). Returns openness [0,1].
    float process(float decay01, float pitchFactor, float attackTau, float ctrl01, float open01,
                  float hitV) {
        const bool hitHigh = hitV >= kHitThresholdV;
        if (hitHigh && !prevHitHigh) {
            eFast += cfg.hitBump;
            eSlow += cfg.hitBump;
        }
        prevHitHigh = hitHigh;

        const float dFast = std::exp(-1.f / (cfg.fastTau * sr));
        const float tauSlow = slowTau(decay01, pitchFactor);
        const float dSlow = std::exp(-1.f / (tauSlow * sr));
        eFast *= dFast;
        eSlow *= dSlow;

        const float target = eFast + eSlow + std::max(ctrl01, 0.f);
        const float a = 1.f - std::exp(-1.f / (std::max(attackTau, 1.0e-5f) * sr));
        if (target > env)
            env += (target - env) * a;
        else
            env = target;

        // Flush denormal tails so the FPU doesn't stall on a long decay.
        eFast = flushDenorm(eFast);
        eSlow = flushDenorm(eSlow);
        env = flushDenorm(env);

        return clampf(std::max(env, open01), 0.f, 1.f);
    }

  private:
    float slowTau(float decay01, float pitchFactor) const {
        const float d = clampf(decay01, 0.f, 1.f);
        return cfg.slowMin * std::pow(cfg.slowMax / cfg.slowMin, d) * pitchFactor;
    }

    float sr;
    Config cfg;
    float eFast;
    float eSlow;
    float env;
    bool prevHitHigh;
};

// TPT state-variable low-pass used as the Strike gate. Cutoff sweeps
// exponentially from the floor to the ceiling as control goes 0 -> 1.
class Gate {
  public:
    // Cutoff sweep endpoints and resonance. Defaults are golden-preserving.
    struct Config {
        float floorHz;   // Hz — cutoff at control=0 (and the hard floor)
        float q;         // -- — filter Q (resonance)

        Config() : floorHz(20.f), q(0.707f) {
        }
    };

    explicit Gate(float sampleRate = 48000.f) : sr(sampleRate), cfg(), ic1eq(0.f), ic2eq(0.f) {
    }

    void setSampleRate(float sampleRate) {
        sr = std::max(sampleRate, 1000.f);
    }

    float getSampleRate() const {
        return sr;
    }

    void setConfig(const Config& c) {
        cfg = c;
    }
    const Config& getConfig() const {
        return cfg;
    }

    void reset() {
        ic1eq = 0.f;
        ic2eq = 0.f;
    }

    // audio: input sample; control [0,1] openness; cutoffCeiling (Hz) the cutoff
    // at full openness. Returns the gated audio.
    float process(float audio, float control, float cutoffCeiling) {
        const float c = clampf(control, 0.f, 1.f);
        const float fc =
            clampf(cfg.floorHz * std::pow(cutoffCeiling / cfg.floorHz, c), cfg.floorHz, 0.45f * sr);
        const float g = std::tan(kPi * fc / sr);
        const float k = 1.f / cfg.q;
        const float a1 = 1.f / (1.f + g * (g + k));
        const float a2 = g * a1;
        const float a3 = g * a2;

        const float v3 = audio - ic2eq;
        const float v1 = a1 * ic1eq + a2 * v3;
        const float v2 = ic2eq + a2 * ic1eq + a3 * v3;
        ic1eq = flushDenorm(2.f * v1 - ic1eq);
        ic2eq = flushDenorm(2.f * v2 - ic2eq);
        return c * v2;
    }

  private:
    float sr;
    Config cfg;
    float ic1eq;
    float ic2eq;
};

// Crude monophonic pitch tracker (zero-crossing + envelope gate). estHz is
// clamped to [20, 12000] Hz and only updates above a small signal floor.
class PitchTracker {
  public:
    explicit PitchTracker(float sampleRate = 48000.f)
        : sr(sampleRate), prev(0.f), samplesSinceCross(0.f), env(0.f), estHzValue(110.f) {
    }

    void setSampleRate(float sampleRate) {
        sr = std::max(sampleRate, 1000.f);
    }

    float getSampleRate() const {
        return sr;
    }

    void reset() {
        prev = 0.f;
        samplesSinceCross = 0.f;
        env = 0.f;
        estHzValue = 110.f;
    }

    // Current pitch estimate in Hz, in [20, 12000].
    float estHz() const {
        return estHzValue;
    }

    // x: input audio sample. Returns the updated pitch estimate (Hz).
    float process(float x) {
        const float a = std::fabs(x);
        env += (a > env ? 0.05f : 0.0008f) * (a - env);

        samplesSinceCross += 1.f;
        if (env > 0.02f && prev <= 0.f && x > 0.f) {
            const float period = std::max(samplesSinceCross, 1.f);
            const float hz = clampf(sr / period, 20.f, 12000.f);
            estHzValue += 0.10f * (hz - estHzValue);
            samplesSinceCross = 0.f;
        }
        prev = x;
        return estHzValue;
    }

  private:
    float sr;
    float prev;
    float samplesSinceCross;
    float env;
    float estHzValue;
};

// ===== Public voice configuration (Strike imperfection) ====================

// Analogue-dirt configuration for StrikeCore (component tolerances, slow drift,
// and a low noise floor). Disabled by default; deterministic when off.
struct StrikeImperfectionConfig {
    bool enabled;      // master on/off for the dirt layer
    float noiseAmp;    // linear amplitude of the pink noise floor
    float driftAmount; // scales the slow random drift of the decay tail

    StrikeImperfectionConfig() : enabled(false), noiseAmp(1.0e-4f), driftAmount(1.f) {
    }
};

namespace detail {

// ----- StrikeImperfection ---------------------------------------------------
// Seeded analogue-dirt model: per-instance tolerances, a slow random drift, and
// a pink-noise floor. Internal to StrikeCore.
class StrikeImperfection {
  public:
    explicit StrikeImperfection(uint64_t seed = 0x535452494B450000ull)
        : seedValue(seed),
          config(),
          tolDecay(1.f),
          tolCutoff(1.f),
          tolLevel(1.f),
          noiseRng(seed ^ 0x2222222222222222ull),
          driftRng(seed ^ 0x1111111111111111ull),
          counter(0),
          driftDecay(0.f) {
        SplitMix64 rng(seed);
        tolDecay = dev(rng, 0.10f);
        tolCutoff = dev(rng, 0.06f);
        tolLevel = dev(rng, 0.03f);
    }

    void setConfig(const StrikeImperfectionConfig& cfg) {
        config = cfg;
        reset();
    }

    const StrikeImperfectionConfig& getConfig() const {
        return config;
    }

    void reset() {
        noiseRng.setSeed(seedValue ^ 0x2222222222222222ull);
        driftRng.setSeed(seedValue ^ 0x1111111111111111ull);
        pinkFloor.reset();
        counter = 0;
        driftDecay = 0.f;
    }

    void update() {
        if (!config.enabled)
            return;
        ++counter;
        if (counter < 64)
            return;
        counter = 0;
        const float amt = config.driftAmount;
        driftDecay = clampf(driftDecay * 0.99f + driftRng.nextF32() * 0.0015f * amt, -0.04f, 0.04f);
    }

    void apply(float decay01, float cutoff, float level, float& outDecay, float& outCutoff,
               float& outLevel) const {
        if (!config.enabled) {
            outDecay = decay01;
            outCutoff = cutoff;
            outLevel = level;
            return;
        }
        outDecay = clampf(decay01 * tolDecay + driftDecay, 0.f, 1.f);
        outCutoff = cutoff * tolCutoff;
        outLevel = level * tolLevel;
    }

    float applyOutput(float y) {
        if (!config.enabled || !std::isfinite(config.noiseAmp))
            return y;
        const float p = pinkFloor.next(noiseRng.nextF32());
        return y + p * config.noiseAmp;
    }

  private:
    static float dev(SplitMix64& rng, float pct) {
        return 1.f + (rng.next01() * 2.f - 1.f) * pct;
    }

    uint64_t seedValue;
    StrikeImperfectionConfig config;
    float tolDecay;
    float tolCutoff;
    float tolLevel;
    XorShift noiseRng;
    XorShift driftRng;
    PinkNoise pinkFloor;
    uint32_t counter;
    float driftDecay;
};

} // namespace detail

// ===== Public voice: StrikeCore ============================================
//
// A clean, zero-bleed, envelope-driven low-pass gate. A HIT edge fires a
// percussion envelope (dual-rate), a Material knob shapes attack/cutoff/level,
// and an optional analogue-dirt layer adds tolerances/drift/noise.
// Construct -> setSampleRate -> setParams -> loop processSample.
class StrikeCore {
  public:
    // pitchFactor clamp/center: the slow tail is scaled by
    // sqrt(centerHz / estHz), clamped to [minFactor, maxFactor].
    struct PitchConfig {
        float centerHz;  // Hz — reference pitch (unity factor)
        float floorHz;   // Hz — minimum estHz used in the ratio
        float minFactor; // -- — lower clamp on the factor
        float maxFactor; // -- — upper clamp on the factor

        PitchConfig() : centerHz(220.f), floorHz(20.f), minFactor(0.25f), maxFactor(2.f) {
        }
    };

    explicit StrikeCore(float sampleRate = 48000.f, uint64_t seed = 0x535452494B450000ull)
        : sr(sampleRate),
          params(),
          materialCfg(),
          pitchCfg(),
          envelope(sampleRate),
          gate(sampleRate),
          pitch(sampleRate),
          imperfection(seed),
          lastControlValue(0.f) {
    }

    void setSampleRate(float sampleRate) {
        sr = std::max(sampleRate, 1000.f);
        envelope.setSampleRate(sr);
        gate.setSampleRate(sr);
        pitch.setSampleRate(sr);
    }

    float getSampleRate() const {
        return sr;
    }

    // Typed parameter setter (preferred). open/decay/material clamped to [0,1].
    void setParams(const StrikeParams& p) {
        params = p;
        params.open = clampf(params.open, 0.f, 1.f);
        params.decay = clampf(params.decay, 0.f, 1.f);
        params.material = clampf(params.material, 0.f, 1.f);
    }

    // Positional shim — kept for existing call sites. Forwards into the struct
    // setter. open/decay/material in [0,1].
    void setParams(float open, float decay, float material) {
        StrikeParams p;
        p.open = open;
        p.decay = decay;
        p.material = material;
        setParams(p);
    }

    const StrikeParams& getParams() const {
        return params;
    }

    // Typed imperfection setter (preferred).
    void setImperfection(const StrikeImperfectionConfig& cfg) {
        imperfection.setConfig(cfg);
    }

    // Positional shim — kept for existing call sites. Forwards into the struct
    // setter. enabled on/off; noiseAmp linear floor; driftAmount scale.
    void setImperfection(bool enabled, float noiseAmp, float driftAmount) {
        StrikeImperfectionConfig cfg;
        cfg.enabled = enabled;
        cfg.noiseAmp = noiseAmp;
        cfg.driftAmount = driftAmount;
        setImperfection(cfg);
    }

    const StrikeImperfectionConfig& getImperfection() const {
        return imperfection.getConfig();
    }

    // Opt-in tone-shaping endpoints (defaults preserve the golden sound).
    void setMaterialConfig(const Material::Config& c) {
        materialCfg = c;
    }
    const Material::Config& getMaterialConfig() const {
        return materialCfg;
    }
    void setPitchConfig(const PitchConfig& c) {
        pitchCfg = c;
    }
    const PitchConfig& getPitchConfig() const {
        return pitchCfg;
    }
    void setEnvelopeConfig(const Envelope::Config& c) {
        envelope.setConfig(c);
    }
    const Envelope::Config& getEnvelopeConfig() const {
        return envelope.getConfig();
    }
    void setGateConfig(const Gate::Config& c) {
        gate.setConfig(c);
    }
    const Gate::Config& getGateConfig() const {
        return gate.getConfig();
    }

    void reset() {
        envelope.reset();
        gate.reset();
        pitch.reset();
        imperfection.reset();
        lastControlValue = 0.f;
    }

    // Meter read-back: the last gate openness [0,1]. lastControl() is the
    // canonical name; getMeter() is the common cross-voice accessor.
    float lastControl() const {
        return lastControlValue;
    }
    float getMeter() const {
        return lastControlValue;
    }

    // audioIn: +-1 nominal audio; ctrl01 [0,1] sustain; decayMod additive decay
    // (added to params.decay, clamped); hitV gate volts (rising edge fires).
    // Non-finite inputs are scrubbed to 0. Returns the gated audio sample.
    float processSample(float audioIn, float ctrl01, float decayMod, float hitV) {
        if (!std::isfinite(audioIn))
            audioIn = 0.f;
        if (!std::isfinite(ctrl01))
            ctrl01 = 0.f;
        if (!std::isfinite(decayMod))
            decayMod = 0.f;
        if (!std::isfinite(hitV))
            hitV = 0.f;

        imperfection.update();

        const Material mat = Material::from01(params.material, materialCfg);
        const float decayEff = clampf(params.decay + decayMod, 0.f, 1.f);

        float impDecay = decayEff;
        float cutoffCeiling = mat.cutoffCeiling;
        float level = mat.level;
        imperfection.apply(decayEff, mat.cutoffCeiling, mat.level, impDecay, cutoffCeiling, level);

        const float estHz = pitch.process(audioIn);
        const float pf = pitchFactor(estHz);

        const float control =
            envelope.process(impDecay, pf, mat.attackTau, ctrl01, params.open, hitV);
        lastControlValue = control;

        const float y = gate.process(audioIn, control, cutoffCeiling) * level;
        return imperfection.applyOutput(y);
    }

    // Block helper: a trivially-correct loop over processSample. All buffers are
    // length n; pass null cv/decay/hit to use zero for that input.
    void processBlock(const float* in, const float* ctrl01, const float* decayMod,
                      const float* hitV, float* out, int n) {
        for (int i = 0; i < n; ++i) {
            out[i] = processSample(in[i], ctrl01 ? ctrl01[i] : 0.f, decayMod ? decayMod[i] : 0.f,
                                   hitV ? hitV[i] : 0.f);
        }
    }

    bool isFinite() const {
        return std::isfinite(lastControlValue);
    }

    // Reset only the recursive processing state (params/configs intact).
    void scrubState() {
        envelope.reset();
        gate.reset();
        pitch.reset();
        imperfection.reset();
        lastControlValue = 0.f;
    }

  private:
    float pitchFactor(float estHz) const {
        return clampf(std::sqrt(pitchCfg.centerHz / std::max(estHz, pitchCfg.floorHz)),
                      pitchCfg.minFactor, pitchCfg.maxFactor);
    }

    float sr;
    StrikeParams params;
    Material::Config materialCfg;
    PitchConfig pitchCfg;
    Envelope envelope;
    Gate gate;
    PitchTracker pitch;
    detail::StrikeImperfection imperfection;
    float lastControlValue;
};

} // namespace sbank
