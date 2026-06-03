// Native VCV Rack adapter for the S-Bank vactrol low-pass gate.
//
// The publishable Rack path is pure C++: one DSP core per polyphony channel, no
// Rust staticlib or C ABI in the plugin build.

#include "plugin.hpp"
#include "dsp/SBankDSP.hpp"

#include <cmath>

static const int MAX_CHANNELS = 16;

struct VactrolLPG : SBankModule {
    enum ParamId {
        RESONANCE_PARAM,
        DRIVE_PARAM,
        MODE_PARAM,        // 0 = Both, 1 = VCA, 2 = Lowpass
        OVERSAMPLE_PARAM,  // 1, 2, 4
        PARAMS_LEN
    };
    enum InputId {
        AUDIO_INPUT,
        CV_INPUT,
        PARAMS_INPUT_LEN
    };
    enum OutputId {
        AUDIO_OUTPUT,
        OUTPUTS_LEN
    };
    enum LightId { LIGHTS_LEN };

    sbank::VactrolLpg voices[MAX_CHANNELS];

    VactrolLPG() {
        config(PARAMS_LEN, PARAMS_INPUT_LEN, OUTPUTS_LEN, LIGHTS_LEN);
        configParam(RESONANCE_PARAM, 0.f, 1.f, 0.2f, "Resonance");
        configParam(DRIVE_PARAM, 0.f, 8.f, 1.f, "Drive");
        configSwitch(MODE_PARAM, 0.f, 2.f, 0.f, "Mode", {"Both", "VCA", "Lowpass"});
        configSwitch(OVERSAMPLE_PARAM, 0.f, 2.f, 1.f, "Oversampling", {"1x", "2x", "4x"});
        configInput(AUDIO_INPUT, "Audio");
        configInput(CV_INPUT, "Control voltage");
        configOutput(AUDIO_OUTPUT, "Audio");

        float sr = APP->engine->getSampleRate();
        for (int c = 0; c < MAX_CHANNELS; c++) {
            voices[c].setSampleRate(sr);
        }
    }

    void onSampleRateChange(const SampleRateChangeEvent& e) override {
        for (int c = 0; c < MAX_CHANNELS; c++) {
            voices[c].setSampleRate(e.sampleRate);
        }
    }

    void process(const ProcessArgs& args) override {
        int channels = std::max(1, inputs[AUDIO_INPUT].getChannels());
        outputs[AUDIO_OUTPUT].setChannels(channels);

        uint32_t mode = (uint32_t) std::round(params[MODE_PARAM].getValue());
        float resonance = params[RESONANCE_PARAM].getValue();
        float drive = params[DRIVE_PARAM].getValue();
        // Map the 0/1/2 switch to the 1/2/4 oversampling factor.
        uint32_t osIndex = (uint32_t) std::round(params[OVERSAMPLE_PARAM].getValue());
        uint32_t oversample = (osIndex == 0) ? 1 : (osIndex == 1) ? 2 : 4;

        for (int c = 0; c < channels; c++) {
            voices[c].setParams(mode, resonance, 0.f, drive, oversample);
            float audio = inputs[AUDIO_INPUT].getPolyVoltage(c) / 5.f; // +-5V -> +-1
            float cv = inputs[CV_INPUT].getPolyVoltage(c);
            float y = voices[c].processSample(audio, cv);
            outputs[AUDIO_OUTPUT].setVoltage(y * 5.f, c);
        }
    }
};

struct VactrolLPGWidget : SBankModuleWidget {
    VactrolLPGWidget(VactrolLPG* module) {
        setModule(module);
        loadPanels("VactrolLPG");  // black + silver; right-click to toggle.
#include "VactrolLPG_panel.inc"
    }
};

Model* modelVactrolLPG = createModel<VactrolLPG, VactrolLPGWidget>("VactrolLPG");
