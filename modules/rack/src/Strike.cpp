// SPDX-License-Identifier: GPL-3.0-or-later
// S-Bank "Strike" — a clean, zero-bleed, envelope-driven low-pass gate.
//
// A dual module (channels A/B), each POLYPHONIC (up to 16 voices), driving the native
// C++ Strike DSP. Per channel: OPEN / DECAY faders, a MATERIAL knob, DEC / CTRL CV
// attenuverters, IN / HIT / DEC / CTRL inputs, OUT, and an openness LED.
//
// The analogue-dirt layer is ALWAYS ON — part of the instrument's voice, not an option.
// A right-click "Deterministic" item disables it for reproducible/stacking use.
//
// CTRL is normalled to +10 V (attenuverter at 0 ⇒ no effect). IN is normalled to a DC
// level, so a HIT with nothing patched into IN emits the raw envelope (a percussive ping).

#include "plugin.hpp"
#include "dsp/SBankDSP.hpp"

static const int MAX_POLY = 16;
static const int METER_SEGS = 7;   // openness-meter segments per channel (between the faders)

struct Strike : SBankModule {
    enum ParamId {
        A_OPEN_PARAM, A_DECAY_PARAM, A_MATERIAL_PARAM, A_DECAYCV_PARAM, A_CTRLCV_PARAM,
        B_OPEN_PARAM, B_DECAY_PARAM, B_MATERIAL_PARAM, B_DECAYCV_PARAM, B_CTRLCV_PARAM,
        PARAMS_LEN
    };
    enum InputId {
        A_IN_INPUT, A_HIT_INPUT, A_DECAY_INPUT, A_CTRL_INPUT,
        B_IN_INPUT, B_HIT_INPUT, B_DECAY_INPUT, B_CTRL_INPUT,
        INPUTS_LEN
    };
    enum OutputId { A_OUT_OUTPUT, B_OUT_OUTPUT, OUTPUTS_LEN };
    enum LightId {
        A_METER_LIGHT,                          // 5 segments (index 0 = bottom)
        B_METER_LIGHT = A_METER_LIGHT + METER_SEGS,
        LIGHTS_LEN = B_METER_LIGHT + METER_SEGS
    };

    sbank::StrikeCore voice[2][MAX_POLY];
    dsp::SchmittTrigger hitTrigger[2][MAX_POLY];  // clean, hysteretic HIT edge per voice
    bool deterministic = false;   // right-click: disable the always-on analogue drift
    int appliedImperfection = -1; // tracks what's pushed to the cores (-1 = unset)

    Strike() {
        config(PARAMS_LEN, INPUTS_LEN, OUTPUTS_LEN, LIGHTS_LEN);
        for (int ch = 0; ch < 2; ch++) {
            int o = ch * 5, io = ch * 4;
            std::string p = ch == 0 ? "Ch A " : "Ch B ";
            configParam(A_OPEN_PARAM + o, 0.f, 1.f, 0.f, p + "Open (gate floor)", "%", 0.f, 100.f);
            configParam(A_DECAY_PARAM + o, 0.f, 1.f, 0.4f, p + "Decay", "%", 0.f, 100.f);
            configParam(A_MATERIAL_PARAM + o, 0.f, 1.f, 0.f, p + "Material (hard→soft)", "%", 0.f, 100.f);
            configParam(A_DECAYCV_PARAM + o, -1.f, 1.f, 0.f, p + "Decay CV amount", "%", 0.f, 100.f);
            configParam(A_CTRLCV_PARAM + o, -1.f, 1.f, 0.f, p + "Ctrl CV amount", "%", 0.f, 100.f);
            configInput(A_IN_INPUT + io, p + "Audio");
            configInput(A_HIT_INPUT + io, p + "Hit — trigger (pings the envelope when In is unpatched)");
            configInput(A_DECAY_INPUT + io, p + "Decay CV");
            configInput(A_CTRL_INPUT + io, p + "Ctrl (normalled +10V)");
            configOutput(A_OUT_OUTPUT + ch, p + "Out");
            configBypass(A_IN_INPUT + io, A_OUT_OUTPUT + ch);  // bypass passes dry IN→OUT
        }

        float sr = APP->engine->getSampleRate();
        for (int ch = 0; ch < 2; ch++)
            for (int c = 0; c < MAX_POLY; c++)
                voice[ch][c].setSampleRate(sr);
    }

    void onSampleRateChange(const SampleRateChangeEvent& e) override {
        for (int ch = 0; ch < 2; ch++)
            for (int c = 0; c < MAX_POLY; c++)
                voice[ch][c].setSampleRate(e.sampleRate);
    }

    void onReset(const ResetEvent& e) override {
        Module::onReset(e);
        for (int ch = 0; ch < 2; ch++)
            for (int c = 0; c < MAX_POLY; c++)
                voice[ch][c].reset();
    }

    void processChannel(int ch, const ProcessArgs& args) {
        int po = ch * 5, io = ch * 4;
        Input& in = inputs[A_IN_INPUT + io];
        Input& hitIn = inputs[A_HIT_INPUT + io];
        Input& decIn = inputs[A_DECAY_INPUT + io];
        Input& ctrlIn = inputs[A_CTRL_INPUT + io];
        Output& out = outputs[A_OUT_OUTPUT + ch];

        // Polyphony for the channel is driven by its audio IN and HIT gate.
        int channels = std::max(1, std::max(in.getChannels(), hitIn.getChannels()));
        out.setChannels(channels);

        float open = params[A_OPEN_PARAM + po].getValue();
        float decay = params[A_DECAY_PARAM + po].getValue();
        float material = params[A_MATERIAL_PARAM + po].getValue();
        float decayAtten = params[A_DECAYCV_PARAM + po].getValue();
        float ctrlAtten = params[A_CTRLCV_PARAM + po].getValue();
        bool inConn = in.isConnected();
        bool decConn = decIn.isConnected();
        bool ctrlConn = ctrlIn.isConnected();

        for (int c = 0; c < channels; c++) {
            sbank::StrikeCore& v = voice[ch][c];
            v.setParams(open, decay, material);

            // CTRL: normalled to +10 V, scaled by its attenuverter, mapped to 0..1.
            float ctrlV = (ctrlConn ? ctrlIn.getPolyVoltage(c) : 10.f) * ctrlAtten;
            float ctrl01 = clamp(ctrlV / 10.f, 0.f, 1.f);

            // DECAY CV: attenuverted, mapped to 0..1 units, additive (0 when unpatched).
            float decayMod = decConn ? decIn.getPolyVoltage(c) * decayAtten / 10.f : 0.f;

            // IN normalled to DC (+1.0 = +5 V) so a HIT pings the envelope out.
            float audio = inConn ? in.getPolyVoltage(c) / 5.f : 1.0f;

            // Clean, hysteretic HIT edge so a noisy or slow gate can't chatter-retrigger.
            hitTrigger[ch][c].process(hitIn.getPolyVoltage(c), 0.1f, 1.f);
            float hit = hitTrigger[ch][c].isHigh() ? 10.f : 0.f;

            out.setVoltage(v.processSample(audio, ctrl01, decayMod, hit) * 5.f, c);
        }
        // Openness meter (first voice): a ladder that fills with gate openness and falls
        // at the decay rate, so you watch the strike ring out. Bottom segment lights first.
        float openness = voice[ch][0].lastControl();
        int lbase = (ch == 0) ? A_METER_LIGHT : B_METER_LIGHT;
        for (int i = 0; i < METER_SEGS; i++)
            lights[lbase + i].setBrightness(clamp(openness * METER_SEGS - i, 0.f, 1.f));
    }

    void process(const ProcessArgs& args) override {
        // Imperfection is on unless the user opts into deterministic mode.
        int want = deterministic ? 0 : 1;
        if (want != appliedImperfection) {
            for (int ch = 0; ch < 2; ch++)
                for (int c = 0; c < MAX_POLY; c++)
                    voice[ch][c].setImperfection(!deterministic, 1.0e-4f, 1.f);
            appliedImperfection = want;
        }
        processChannel(0, args);
        processChannel(1, args);
    }

    json_t* dataToJson() override {
        json_t* root = SBankModule::dataToJson();   // persists panel finish
        json_object_set_new(root, "deterministic", json_boolean(deterministic));
        return root;
    }
    void dataFromJson(json_t* root) override {
        SBankModule::dataFromJson(root);
        if (json_t* d = json_object_get(root, "deterministic"))
            deterministic = json_boolean_value(d);
    }
};

struct StrikeWidget : SBankModuleWidget {
    StrikeWidget(Strike* module) {
        setModule(module);
        loadPanels("Strike");  // black + silver; toggle via right-click. Component
        // placement is generated from the panel spec (tools/panelgen) — zero drift.
#include "Strike_panel.inc"
    }

    void appendContextMenu(Menu* menu) override {
        SBankModuleWidget::appendContextMenu(menu);  // panel finish
        if (auto* m = dynamic_cast<Strike*>(module))
            menu->addChild(createBoolPtrMenuItem(
                "Deterministic (disable analog drift)", "", &m->deterministic));
    }
};

Model* modelStrike = createModel<Strike, StrikeWidget>("Strike");
