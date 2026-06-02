// Thin VCV Rack adapter over the Rust vactrol-core C ABI.
//
// The DSP lives entirely in the Rust staticlib; this module owns one opaque core
// instance per polyphony channel and forwards process() per sample. The FFI is
// panic-safe and allocation-free on the audio thread (see ../vactrol_core.h and
// crates/vactrol-core/src/ffi.rs).
//
// Note: the Rust core is currently scalar (one voice per call). When it grows a
// lane-parametric SIMD path, map a `float_4` of four channels onto one core call
// here instead of the per-channel loop below.

#include "plugin.hpp"

extern "C" {
#include "../s_bank.h"
}

static const int MAX_CHANNELS = 16;

struct VactrolLPG : Module {
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

    Lpg* voices[MAX_CHANNELS] = {};

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
            voices[c] = vactrol_lpg_create(sr);
        }
    }

    ~VactrolLPG() override {
        for (int c = 0; c < MAX_CHANNELS; c++) {
            vactrol_lpg_destroy(voices[c]);
        }
    }

    void onSampleRateChange(const SampleRateChangeEvent& e) override {
        for (int c = 0; c < MAX_CHANNELS; c++) {
            vactrol_lpg_set_sample_rate(voices[c], e.sampleRate);
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
            vactrol_lpg_set_params(voices[c], mode, resonance, 0.f, drive, oversample);
            float audio = inputs[AUDIO_INPUT].getPolyVoltage(c) / 5.f; // +-5V -> +-1
            float cv = inputs[CV_INPUT].getPolyVoltage(c);
            float y = vactrol_lpg_process_sample(voices[c], audio, cv);
            outputs[AUDIO_OUTPUT].setVoltage(y * 5.f, c);
        }
    }
};

struct VactrolLPGWidget : ModuleWidget {
    VactrolLPGWidget(VactrolLPG* module) {
        setModule(module);
        setPanel(createPanel(asset::plugin(pluginInstance, "res/VactrolLPG.svg")));

        addParam(createParamCentered<RoundBlackKnob>(mm2px(Vec(10, 20)), module, VactrolLPG::RESONANCE_PARAM));
        addParam(createParamCentered<RoundBlackKnob>(mm2px(Vec(10, 40)), module, VactrolLPG::DRIVE_PARAM));
        addParam(createParamCentered<CKSSThree>(mm2px(Vec(10, 60)), module, VactrolLPG::MODE_PARAM));
        addParam(createParamCentered<CKSSThree>(mm2px(Vec(10, 75)), module, VactrolLPG::OVERSAMPLE_PARAM));

        addInput(createInputCentered<PJ301MPort>(mm2px(Vec(8, 95)), module, VactrolLPG::AUDIO_INPUT));
        addInput(createInputCentered<PJ301MPort>(mm2px(Vec(8, 108)), module, VactrolLPG::CV_INPUT));
        addOutput(createOutputCentered<PJ301MPort>(mm2px(Vec(8, 120)), module, VactrolLPG::AUDIO_OUTPUT));
    }
};

Model* modelVactrolLPG = createModel<VactrolLPG, VactrolLPGWidget>("VactrolLPG");
