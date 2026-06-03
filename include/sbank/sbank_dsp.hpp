// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Stable, canonical include for the S-Bank DSP library:
//
//     #include <sbank/sbank_dsp.hpp>
//
// This is a thin forwarding header. The real implementation lives at
// modules/rack/src/dsp/SBankDSP.hpp (kept in place so the Rack build's include
// path keeps working). To vendor the library into another project, copy the
// real header (and, optionally, this forwarding header) into your tree and add
// the directory containing it to your compiler's include path (-I).
#pragma once

#include "../../modules/rack/src/dsp/SBankDSP.hpp"
