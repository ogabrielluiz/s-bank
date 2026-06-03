// SPDX-License-Identifier: GPL-3.0-or-later
#include "plugin.hpp"

Plugin* pluginInstance;

void init(Plugin* p) {
    pluginInstance = p;
    p->addModel(modelVactrolLPG);
    p->addModel(modelStrike);
}
