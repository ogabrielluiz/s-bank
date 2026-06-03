#include "../src/dsp/SBankDSP.hpp"

#include <cassert>
#include <cmath>

int main() {
    sbank::VactrolLpg lpg(48000.f);
    lpg.setParams(0, 0.2f, 0.f, 1.f, 2);

    float maxLpg = 0.f;
    for (int i = 0; i < 48000; ++i) {
        const float x = std::sin(2.f * sbank::kPi * 220.f * static_cast<float>(i) / 48000.f);
        const float cv = i < 12000 ? 8.f : 0.f;
        const float y = lpg.processSample(x, cv);
        assert(std::isfinite(y));
        maxLpg = std::max(maxLpg, std::fabs(y));
    }
    assert(maxLpg > 0.001f);

    sbank::StrikeCore strike(48000.f);
    strike.setParams(0.f, 0.5f, 0.f);

    float peak = 0.f;
    for (int i = 0; i < 96000; ++i) {
        const float hit = i < 10 ? 5.f : 0.f;
        const float y = strike.processSample(1.f, 0.f, 0.f, hit);
        assert(std::isfinite(y));
        peak = std::max(peak, y);
    }
    assert(peak > 0.1f);

    return 0;
}
