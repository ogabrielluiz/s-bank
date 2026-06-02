#pragma once
#include <rack.hpp>

using namespace rack;

extern Plugin* pluginInstance;

extern Model* modelVactrolLPG;
extern Model* modelStrike;

// Shared base: a switchable panel finish (Black / Silver), persisted per instance.
struct SBankModule : Module {
    int finish = 0;  // 0 = black, 1 = silver

    json_t* dataToJson() override {
        json_t* root = json_object();
        json_object_set_new(root, "finish", json_integer(finish));
        return root;
    }
    void dataFromJson(json_t* root) override {
        if (json_t* f = json_object_get(root, "finish"))
            finish = json_integer_value(f);
    }
};

// Widget base: loads both panel finishes (res/<base>.svg + res/<base>-silver.svg),
// shows the one selected by the module's `finish`, and adds the right-click toggle.
struct SBankModuleWidget : ModuleWidget {
    widget::Widget* panelBlack = nullptr;
    widget::Widget* panelSilver = nullptr;

    void loadPanels(const std::string& base) {
        panelBlack = createPanel(asset::plugin(pluginInstance, "res/" + base + ".svg"));
        setPanel(panelBlack);  // first child, sets box.size
        panelSilver = createPanel(asset::plugin(pluginInstance, "res/" + base + "-silver.svg"));
        panelSilver->visible = false;
        addChild(panelSilver);
        // Silver screws on both finishes (classic metallic hardware look).
        float x0 = RACK_GRID_WIDTH;
        float x1 = box.size.x - 2 * RACK_GRID_WIDTH;
        float y1 = RACK_GRID_HEIGHT - RACK_GRID_WIDTH;
        for (math::Vec p : {math::Vec(x0, 0), math::Vec(x1, 0), math::Vec(x0, y1), math::Vec(x1, y1)})
            addChild(createWidget<ScrewSilver>(p));
    }

    void step() override {
        if (module) {
            bool silver = static_cast<SBankModule*>(module)->finish == 1;
            if (panelBlack) panelBlack->visible = !silver;
            if (panelSilver) panelSilver->visible = silver;
        }
        ModuleWidget::step();
    }

    void appendContextMenu(Menu* menu) override {
        if (!module) return;
        SBankModule* m = static_cast<SBankModule*>(module);
        menu->addChild(new MenuSeparator);
        menu->addChild(createIndexPtrSubmenuItem("Panel finish", {"Black", "Silver"}, &m->finish));
    }
};
