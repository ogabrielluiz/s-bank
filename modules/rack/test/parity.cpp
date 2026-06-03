// C++<->Rust DSP parity harness.
//
// Reproduces the Rust golden scenarios with the C++ port and prints the buffer, so the
// C++ DSP can be checked sample-for-sample against the Rust reference (the components/
// crates). Run via test/run_parity.sh.
//
// Cases (argv[1]):
//   lpg:pluck_both | lpg:vca_tone | lpg:lowpass_sweep   (vs testdata/golden/<name>.json)
//   strike:ping | strike:gated | strike:held            (vs testdata/golden/strike_<name>.json)
#include "../src/dsp/SBankDSP.hpp"

#include <cmath>
#include <cstdio>
#include <cstring>
#include <string>

static const float SR = 48000.f;
static const int N = 4800;       // vactrol golden length
static const int NS = 12000;     // strike golden length

static float sine(int i, float hz) { return std::sin(2.f * sbank::kPi * hz * (float)i / SR); }
static void emit(float y) { std::printf("%.9e\n", y); }

int main(int argc, char** argv) {
    if (argc < 2) return 1;
    std::string c = argv[1];

    if (c.rfind("lpg:", 0) == 0) {
        sbank::VactrolLpg lpg(SR);
        const int gate = (int)(SR * 0.005f);
        std::string s = c.substr(4);
        if (s == "pluck_both") {
            lpg.setParams(0, 0.3f, 0.f, 1.f, 2);
            for (int i = 0; i < N; i++) emit(lpg.processSample(sine(i, 220.f), i < gate ? 8.f : 0.f));
        } else if (s == "vca_tone") {
            lpg.setParams(1, 0.f, 0.f, 2.f, 2);
            for (int i = 0; i < N; i++) emit(lpg.processSample(sine(i, 1000.f), 8.f));
        } else if (s == "lowpass_sweep") {
            lpg.setParams(2, 0.5f, 0.f, 1.f, 2);
            for (int i = 0; i < N; i++) emit(lpg.processSample(sine(i, 500.f), 8.f * (float)i / (float)N));
        } else return 2;
        return 0;
    }

    if (c.rfind("strike:", 0) == 0) {
        sbank::StrikeCore st(SR);
        std::string s = c.substr(7);
        if (s == "ping") {
            st.setParams(0.f, 0.5f, 0.f);
            for (int i = 0; i < NS; i++) emit(st.processSample(1.f, 0.f, 0.f, i < 10 ? 5.f : 0.f));
        } else if (s == "gated") {
            st.setParams(0.f, 0.6f, 0.f);
            for (int i = 0; i < NS; i++) emit(st.processSample(sine(i, 220.f), 0.f, 0.f, i < 10 ? 5.f : 0.f));
        } else if (s == "held") {
            st.setParams(0.f, 0.5f, 1.f);
            for (int i = 0; i < NS; i++) emit(st.processSample(sine(i, 330.f), 1.f, 0.f, 0.f));
        } else return 2;
        return 0;
    }
    return 1;
}
