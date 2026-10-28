// The gallery's made-up examples — loaded by demo/gallery.html only, never
// by src/sign.html, so no published page carries them (cs-20: the page used
// to ship a "source verified" StakePool at an address nothing lives at).
//
// They stand for ladder level 2 — a contract whose source is verified and
// whose every parameter decodes, but which nobody described — which the real
// page has no example of.
(function (ns) {
  'use strict';

  ns.registry.addContract(1, '0x5f2b8c3a9d1e4f6a7b8c9d0e1f2a3b4c5d6e7f80', { name: 'StakePool', verified: true, descriptor: false });
  ns.registry.addDescriptor({
    signature: 'stake(uint256,uint256,address)',
    // Deliberately NO intent: "source verified, no descriptor", so the ladder
    // falls to level 2 and speaks in facts, not meaning.
    abiOnly: true,
    contracts: { 1: { '0x5f2b8c3a9d1e4f6a7b8c9d0e1f2a3b4c5d6e7f80': {} } },
    fields: [],
  });
})(window.VelaCS);
