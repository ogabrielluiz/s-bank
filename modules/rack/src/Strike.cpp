// S-Bank "Strike" — a clean, zero-bleed, envelope-driven low-pass gate.
//
// A dual, mono-per-channel demo module that drives the strike-core DSP (via the
// s-bank plugin C ABI). Each channel: OPEN / DECAY / MATERIAL knobs, DECAY-CV and
// CTRL attenuverters, IN / HIT / DECAY / CTRL inputs, OUT, and an openness LED.
// A global IMPERFECTION switch engages the optional analogue-dirt layer.
//
// CTRL is normalled to +10 V (attenuverter at 0 ⇒ no effect). IN is normalled to a
// DC level, so a HIT with nothing patched into IN emits the raw envelope (ping).

#include "plugin.hpp"

extern "C" {
#include "../s_bank.h"
}

struct Strike : Module {
    enum ParamId {
        A_OPEN_PARAM, A_DECAY_PARAM, A_MATERIAL_PARAM, A_DECAYCV_PARAM, A_CTRLCV_PARAM,
        B_OPEN_PARAM, B_DECAY_PARAM, B_MATERIAL_PARAM, B_DECAYCV_PARAM, B_CTRLCV_PARAM,
        IMPERFECTION_PARAM,
        PARAMS_LEN
    };
    enum InputId {
        A_IN_INPUT, A_HIT_INPUT, A_DECAY_INPUT, A_CTRL_INPUT,
        B_IN_INPUT, B_HIT_INPUT, B_DECAY_INPUT, B_CTRL_INPUT,
        INPUTS_LEN
    };
    enum OutputId { A_OUT_OUTPUT, B_OUT_OUTPUT, OUTPUTS_LEN };
    enum LightId { A_OPEN_LIGHT, B_OPEN_LIGHT, LIGHTS_LEN };

    StrikeVoice* voice[2] = {nullptr, nullptr};
    bool lastImperfection = false;

    Strike() {
        config(PARAMS_LEN, INPUTS_LEN, OUTPUTS_LEN, LIGHTS_LEN);
        for (int ch = 0; ch < 2; ch++) {
            int o = ch * 5;
            std::string p = ch == 0 ? "Ch A " : "Ch B ";
            configParam(A_OPEN_PARAM + o, 0.f, 1.f, 0.f, p + "Open (gate floor)");
            configParam(A_DECAY_PARAM + o, 0.f, 1.f, 0.4f, p + "Decay");
            configParam(A_MATERIAL_PARAM + o, 0.f, 1.f, 0.f, p + "Material (hard→soft)");
            configParam(A_DECAYCV_PARAM + o, -1.f, 1.f, 0.f, p + "Decay CV amount");
            configParam(A_CTRLCV_PARAM + o, -1.f, 1.f, 0.f, p + "Ctrl amount");
            configInput(A_IN_INPUT + ch * 4, p + "Audio");
            configInput(A_HIT_INPUT + ch * 4, p + "Hit (trigger)");
            configInput(A_DECAY_INPUT + ch * 4, p + "Decay CV");
            configInput(A_CTRL_INPUT + ch * 4, p + "Ctrl (normalled +10V)");
            configOutput(A_OUT_OUTPUT + ch, p + "Out");
        }
        configSwitch(IMPERFECTION_PARAM, 0.f, 1.f, 0.f, "Analog imperfection", {"Off", "On"});

        float sr = APP->engine->getSampleRate();
        voice[0] = strike_create(sr);
        voice[1] = strike_create(sr);
    }

    ~Strike() override {
        strike_destroy(voice[0]);
        strike_destroy(voice[1]);
    }

    void onSampleRateChange(const SampleRateChangeEvent& e) override {
        strike_set_sample_rate(voice[0], e.sampleRate);
        strike_set_sample_rate(voice[1], e.sampleRate);
    }

    void processChannel(int ch, const ProcessArgs& args) {
        StrikeVoice* v = voice[ch];
        int po = ch * 5;
        int io = ch * 4;

        float open = params[A_OPEN_PARAM + po].getValue();
        float decay = params[A_DECAY_PARAM + po].getValue();
        float material = params[A_MATERIAL_PARAM + po].getValue();
        strike_set_params(v, open, decay, material);

        // CTRL: normalled to +10 V, scaled by its attenuverter, mapped to 0..1.
        float ctrlV = inputs[A_CTRL_INPUT + io].isConnected()
                          ? inputs[A_CTRL_INPUT + io].getVoltage()
                          : 10.f;
        ctrlV *= params[A_CTRLCV_PARAM + po].getValue();
        float ctrl01 = clamp(ctrlV / 10.f, 0.f, 1.f);

        // DECAY CV: attenuverted, mapped to 0..1 units, additive (0 when unpatched).
        float decayMod = inputs[A_DECAY_INPUT + io].isConnected()
                             ? inputs[A_DECAY_INPUT + io].getVoltage() *
                                   params[A_DECAYCV_PARAM + po].getValue() / 10.f
                             : 0.f;

        // IN normalled to DC (+1.0 normalized = +5 V) so a HIT pings the envelope out.
        float audio = inputs[A_IN_INPUT + io].isConnected()
                          ? inputs[A_IN_INPUT + io].getVoltage() / 5.f
                          : 1.0f;
        float hit = inputs[A_HIT_INPUT + io].getVoltage();

        float y = strike_process_sample(v, audio, ctrl01, decayMod, hit);
        outputs[A_OUT_OUTPUT + ch].setVoltage(y * 5.f);
        lights[A_OPEN_LIGHT + ch].setBrightnessSmooth(strike_last_control(v), args.sampleTime);
    }

    void process(const ProcessArgs& args) override {
        bool imp = params[IMPERFECTION_PARAM].getValue() > 0.5f;
        if (imp != lastImperfection) {
            strike_set_imperfection(voice[0], imp ? 1 : 0, 1.0e-4f, 1.f);
            strike_set_imperfection(voice[1], imp ? 1 : 0, 1.0e-4f, 1.f);
            lastImperfection = imp;
        }
        processChannel(0, args);
        processChannel(1, args);
    }
};

struct StrikeWidget : ModuleWidget {
    StrikeWidget(Strike* module) {
        setModule(module);
        setPanel(createPanel(asset::plugin(pluginInstance, "res/Strike.svg")));

        // Two mirrored channel columns about the 16HP centre line (40.64 mm).
        const float cx[2] = {22.0f, 59.28f};
        for (int ch = 0; ch < 2; ch++) {
            float x = cx[ch];
            int po = ch * 5;
            int io = ch * 4;
            addParam(createParamCentered<RoundBlackKnob>(mm2px(Vec(x, 18)), module, Strike::A_OPEN_PARAM + po));
            addParam(createParamCentered<RoundBlackKnob>(mm2px(Vec(x, 36)), module, Strike::A_DECAY_PARAM + po));
            addParam(createParamCentered<RoundBlackKnob>(mm2px(Vec(x, 54)), module, Strike::A_MATERIAL_PARAM + po));
            addParam(createParamCentered<Trimpot>(mm2px(Vec(x - 8, 69)), module, Strike::A_DECAYCV_PARAM + po));
            addParam(createParamCentered<Trimpot>(mm2px(Vec(x + 8, 69)), module, Strike::A_CTRLCV_PARAM + po));
            addChild(createLightCentered<MediumLight<YellowLight>>(mm2px(Vec(x, 80)), module, Strike::A_OPEN_LIGHT + ch));
            addInput(createInputCentered<PJ301MPort>(mm2px(Vec(x - 9, 92)), module, Strike::A_IN_INPUT + io));
            addInput(createInputCentered<PJ301MPort>(mm2px(Vec(x + 9, 92)), module, Strike::A_HIT_INPUT + io));
            addInput(createInputCentered<PJ301MPort>(mm2px(Vec(x - 9, 105)), module, Strike::A_DECAY_INPUT + io));
            addInput(createInputCentered<PJ301MPort>(mm2px(Vec(x + 9, 105)), module, Strike::A_CTRL_INPUT + io));
            addOutput(createOutputCentered<PJ301MPort>(mm2px(Vec(x, 118)), module, Strike::A_OUT_OUTPUT + ch));
        }
        addParam(createParamCentered<CKSS>(mm2px(Vec(40.64, 118)), module, Strike::IMPERFECTION_PARAM));
    }
};

Model* modelStrike = createModel<Strike, StrikeWidget>("Strike");
