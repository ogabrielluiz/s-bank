// SPDX-License-Identifier: MIT OR Apache-2.0
#pragma once

#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstddef>

namespace sbank {

static const float kPi = 3.14159265358979323846f;
static const float kR3Vca = 1.0e5f;
static const float kR3Filter = 1.0e6f;

inline float clampf(float v, float lo, float hi) {
    return std::min(std::max(v, lo), hi);
}

enum class LpgMode {
    Both = 0,
    Vca = 1,
    Lowpass = 2,
};

struct LpgParams {
    LpgMode mode;
    float resonance;
    float cvOffset;
    float drive;
    uint32_t oversample;

    LpgParams()
        : mode(LpgMode::Both), resonance(0.2f), cvOffset(0.f), drive(1.f), oversample(2) {
    }

    size_t oversampleFactor() const {
        if (oversample <= 1)
            return 1;
        if (oversample <= 3)
            return 2;
        return 4;
    }
};

struct LpgComponents {
    float c1;
    float c2;
    float c3;
    float rfLawA;
    float rfLawB;
    float rOnMin;
    float rOff;
    float tauAttack;
    float tauDecay;

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

class ControlCoeffs {
  public:
    float ifMin;
    float ifMax;

    ControlCoeffs() {
        const double offsetControl = 0.0;
        const double scaleControl = 1.0;

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

        const double offset = 0.9999 * offsetControl + 0.0001;
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
    const float k0 = 146.8f;
    const float k1 = 0.49202f;
    const float k2 = 4.1667e-4f;
    const float k3 = 7.3915e-9f;
};

class ControlPath {
  public:
    explicit ControlPath(float sampleRate = 48000.f) : cvState(0.f), smooth(0.f) {
        setSampleRate(sampleRate);
    }

    void setSampleRate(float sampleRate) {
        smooth = std::exp(-1.f / (0.0015f * std::max(sampleRate, 1.f)));
    }

    void reset() {
        cvState = 0.f;
    }

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

class VactrolModel {
  public:
    explicit VactrolModel(float sampleRate = 48000.f) : sr(sampleRate), rf(LpgComponents().rOff) {
    }

    void setSampleRate(float sampleRate) {
        sr = std::max(sampleRate, 1.f);
    }

    void reset(const LpgComponents& comp) {
        rf = comp.rOff;
    }

    float resistance() const {
        return rf;
    }

    float process(float ifCurrent, const LpgComponents& comp) {
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
    float rf;
};

static const size_t kHalfbandLen = 61;
static const size_t kHalfbandEvenLen = 31;
static const size_t kHalfbandOddLen = 30;

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

class Cell292 {
  public:
    Cell292()
        : f(0.5f / 48000.f),
          rf(1.0e6f),
          r3(kR3Filter),
          c1(1.0e-9f),
          c2(220.0e-12f),
          c3(4.7e-9f),
          resonance(0.f),
          drive(1.f),
          sx(0.f),
          so(0.f),
          sd(0.f),
          xo(0.f) {
    }

    void reset() {
        sx = 0.f;
        so = 0.f;
        sd = 0.f;
        xo = 0.f;
    }

    float solveStep(float x) {
        const float a1 = 1.f / (c1 * rf);
        const float a2 = -(1.f / rf + 1.f / r3) / c1;
        const float b1 = 1.f / (rf * c2);
        const float b2 = -2.f / (rf * c2);
        const float b3 = 1.f / (rf * c2);
        const float b4 = c3 / c2;
        const float d2 = -1.f;

        float d1 = 0.f;
        if (c3 > 0.f) {
            const float amax = (2.f * c1 * r3 + (c2 + c3) * (r3 + rf)) / (c3 * r3);
            d1 = clampf(resonance, 0.f, 1.f) * amax;
        }

        float gx = xo;
        float s2 = 1.f;
        if (drive > 0.f) {
            const float t = std::tanh(drive * xo);
            gx = t / drive;
            s2 = 1.f - t * t;
        }

        const float dx = 1.f / (1.f - b2 * f);
        const float do_ = 1.f / (1.f - a2 * f);
        const float dmas =
            1.f / (1.f - dx * (f * f * b3 * do_ * a1 + b4 * f * d1 * s2 * do_ * a1 + b4 * d2));
        const float nl = d1 * (gx - xo * s2);

        const float yx =
            (sx + f * b1 * x + f * b3 * do_ * so + f * b4 * (sd + (1.f / f) * nl) +
             b4 * d1 * s2 * do_ * so) *
            dx * dmas;
        const float yo = (so + f * a1 * yx) * do_;
        const float yd = (sd + (1.f / f) * nl) + (1.f / f) * (d1 * s2 * yo + d2 * yx);

        sx += 2.f * f * (b1 * x + b2 * yx + b3 * yo + b4 * yd);
        so += 2.f * f * (a1 * yx + a2 * yo);
        if (c3 > 0.f) {
            sd = -(sd + (2.f / f) * nl) - (2.f / f) * (d1 * s2 * yo + d2 * yx);
        }
        else {
            sd = 0.f;
        }
        xo = yo;
        return yo;
    }

    float f;
    float rf;
    float r3;
    float c1;
    float c2;
    float c3;
    float resonance;
    float drive;

  private:
    float sx;
    float so;
    float sd;
    float xo;
};

class AudioPath {
  public:
    explicit AudioPath(float sampleRate = 48000.f) : sr(sampleRate) {
    }

    void setSampleRate(float sampleRate) {
        sr = std::max(sampleRate, 1.f);
    }

    void reset() {
        cell.reset();
        oversampler.reset();
    }

    float process(float x, float rf, const LpgParams& params, const LpgComponents& comp) {
        cell.rf = std::max(rf, 1.f);
        cell.c1 = comp.c1;
        cell.c2 = comp.c2;
        if (params.mode == LpgMode::Vca) {
            cell.c3 = 0.f;
            cell.r3 = kR3Vca;
        }
        else {
            cell.c3 = comp.c3;
            cell.r3 = kR3Filter;
        }
        cell.resonance = params.resonance;
        cell.drive = params.drive;

        const size_t factor = params.oversampleFactor();
        cell.f = 0.5f / (sr * static_cast<float>(factor));
        if (factor == 1)
            return cell.solveStep(x);

        auto solver = [this](float xs) -> float {
            return cell.solveStep(xs);
        };
        return oversampler.process(x, factor, solver);
    }

  private:
    float sr;
    Cell292 cell;
    Oversampler oversampler;
};

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
    }

    void setSampleRate(float sampleRate) {
        sr = std::max(sampleRate, 1.f);
        control.setSampleRate(sr);
        vactrol.setSampleRate(sr);
        audio.setSampleRate(sr);
    }

    void setParams(uint32_t mode, float resonance, float cvOffset, float drive, uint32_t oversample) {
        params.mode = mode == 1 ? LpgMode::Vca : mode == 2 ? LpgMode::Lowpass : LpgMode::Both;
        params.resonance = resonance;
        params.cvOffset = cvOffset;
        params.drive = drive;
        params.oversample = std::min<uint32_t>(oversample, 4);
    }

    void reset() {
        control.reset();
        vactrol.reset(comp);
        audio.reset();
        lastRf = comp.rOff;
    }

    float processSample(float audioIn, float cvIn) {
        const float current = control.process(cvIn, params.cvOffset);
        const float rf = vactrol.process(current, comp);
        lastRf = rf;
        return audio.process(audioIn, rf, params, comp);
    }

    float lastResistance() const {
        return lastRf;
    }

  private:
    float sr;
    LpgParams params;
    LpgComponents comp;
    ControlPath control;
    VactrolModel vactrol;
    AudioPath audio;
    float lastRf;
};

class PinkNoise {
  public:
    PinkNoise() {
        reset();
    }

    void reset() {
        b.fill(0.f);
    }

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

    float nextF32() {
        return (static_cast<float>(nextU32()) / 4294967295.0f) * 2.f - 1.f;
    }

  private:
    uint64_t state;
};

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

    float next01() {
        return static_cast<float>((next() >> 40) & 0xFFFFFFull) / 16777216.0f;
    }

  private:
    uint64_t state;
};

struct StrikeParams {
    float open;
    float decay;
    float material;

    StrikeParams() : open(0.f), decay(0.4f), material(0.f) {
    }
};

struct Material {
    float attackTau;
    float cutoffCeiling;
    float level;

    static Material from01(float material) {
        const float m = clampf(material, 0.f, 1.f);
        Material out;
        out.attackTau = 0.0005f * std::pow(25.0f / 0.5f, m);
        out.cutoffCeiling = 18000.f * std::pow(2500.f / 18000.f, m);
        out.level = 1.f - 0.4f * m;
        return out;
    }
};

class Envelope {
  public:
    explicit Envelope(float sampleRate = 48000.f)
        : sr(sampleRate), eFast(0.f), eSlow(0.f), env(0.f), prevHitHigh(false) {
    }

    void setSampleRate(float sampleRate) {
        sr = std::max(sampleRate, 1.f);
    }

    void reset() {
        eFast = 0.f;
        eSlow = 0.f;
        env = 0.f;
        prevHitHigh = false;
    }

    float process(float decay01, float pitchFactor, float attackTau, float ctrl01, float open01, float hitV) {
        const bool hitHigh = hitV >= 0.25f;
        if (hitHigh && !prevHitHigh) {
            eFast += 0.35f;
            eSlow += 0.35f;
        }
        prevHitHigh = hitHigh;

        const float dFast = std::exp(-1.f / (0.012f * sr));
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

        return clampf(std::max(env, open01), 0.f, 1.f);
    }

  private:
    static float slowTau(float decay01, float pitchFactor) {
        const float d = clampf(decay01, 0.f, 1.f);
        return 0.030f * std::pow(3.5f / 0.030f, d) * pitchFactor;
    }

    float sr;
    float eFast;
    float eSlow;
    float env;
    bool prevHitHigh;
};

class Gate {
  public:
    explicit Gate(float sampleRate = 48000.f) : sr(sampleRate), ic1eq(0.f), ic2eq(0.f) {
    }

    void setSampleRate(float sampleRate) {
        sr = std::max(sampleRate, 1.f);
    }

    void reset() {
        ic1eq = 0.f;
        ic2eq = 0.f;
    }

    float process(float audio, float control, float cutoffCeiling) {
        const float c = clampf(control, 0.f, 1.f);
        const float fc = clampf(20.f * std::pow(cutoffCeiling / 20.f, c), 20.f, 0.45f * sr);
        const float g = std::tan(kPi * fc / sr);
        const float k = 1.f / 0.707f;
        const float a1 = 1.f / (1.f + g * (g + k));
        const float a2 = g * a1;
        const float a3 = g * a2;

        const float v3 = audio - ic2eq;
        const float v1 = a1 * ic1eq + a2 * v3;
        const float v2 = ic2eq + a2 * ic1eq + a3 * v3;
        ic1eq = 2.f * v1 - ic1eq;
        ic2eq = 2.f * v2 - ic2eq;
        return c * v2;
    }

  private:
    float sr;
    float ic1eq;
    float ic2eq;
};

class PitchTracker {
  public:
    explicit PitchTracker(float sampleRate = 48000.f)
        : sr(sampleRate), prev(0.f), samplesSinceCross(0.f), env(0.f), estHzValue(110.f) {
    }

    void setSampleRate(float sampleRate) {
        sr = std::max(sampleRate, 1.f);
    }

    void reset() {
        prev = 0.f;
        samplesSinceCross = 0.f;
        env = 0.f;
        estHzValue = 110.f;
    }

    float estHz() const {
        return estHzValue;
    }

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

struct StrikeImperfectionConfig {
    bool enabled;
    float noiseAmp;
    float driftAmount;

    StrikeImperfectionConfig() : enabled(false), noiseAmp(1.0e-4f), driftAmount(1.f) {
    }
};

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

    void apply(float decay01, float cutoff, float level, float& outDecay, float& outCutoff, float& outLevel) const {
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

class StrikeCore {
  public:
    explicit StrikeCore(float sampleRate = 48000.f, uint64_t seed = 0x535452494B450000ull)
        : sr(sampleRate),
          params(),
          envelope(sampleRate),
          gate(sampleRate),
          pitch(sampleRate),
          imperfection(seed),
          lastControlValue(0.f) {
    }

    void setSampleRate(float sampleRate) {
        sr = std::max(sampleRate, 1.f);
        envelope.setSampleRate(sr);
        gate.setSampleRate(sr);
        pitch.setSampleRate(sr);
    }

    void setParams(float open, float decay, float material) {
        params.open = open;
        params.decay = decay;
        params.material = material;
    }

    void setImperfection(bool enabled, float noiseAmp, float driftAmount) {
        StrikeImperfectionConfig cfg;
        cfg.enabled = enabled;
        cfg.noiseAmp = noiseAmp;
        cfg.driftAmount = driftAmount;
        imperfection.setConfig(cfg);
    }

    void reset() {
        envelope.reset();
        gate.reset();
        pitch.reset();
        imperfection.reset();
        lastControlValue = 0.f;
    }

    float lastControl() const {
        return lastControlValue;
    }

    float processSample(float audioIn, float ctrl01, float decayMod, float hitV) {
        imperfection.update();

        const Material mat = Material::from01(params.material);
        const float decayEff = clampf(params.decay + decayMod, 0.f, 1.f);

        float impDecay = decayEff;
        float cutoffCeiling = mat.cutoffCeiling;
        float level = mat.level;
        imperfection.apply(decayEff, mat.cutoffCeiling, mat.level, impDecay, cutoffCeiling, level);

        const float estHz = pitch.process(audioIn);
        const float pf = pitchFactor(estHz);

        const float control = envelope.process(impDecay, pf, mat.attackTau, ctrl01, params.open, hitV);
        lastControlValue = control;

        const float y = gate.process(audioIn, control, cutoffCeiling) * level;
        return imperfection.applyOutput(y);
    }

  private:
    static float pitchFactor(float estHz) {
        return clampf(std::sqrt(220.f / std::max(estHz, 20.f)), 0.25f, 2.f);
    }

    float sr;
    StrikeParams params;
    Envelope envelope;
    Gate gate;
    PitchTracker pitch;
    StrikeImperfection imperfection;
    float lastControlValue;
};

} // namespace sbank
