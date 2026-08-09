// Anchor migration hook. Runs only through `anchor migrate`, which sends
// transactions to a cluster.
//
// Intentionally empty. No deployment or initialization step is automated here
// until the user supplies a deploy keypair path, its pubkey and an explicit
// cluster approval. Do not add an auto-deploy or auto-init step to this file.

const anchor = require("@coral-xyz/anchor");

module.exports = async function (provider: anchor.AnchorProvider) {
  anchor.setProvider(provider);
  // Phase 8: initialize_vault call goes here, once deployment is approved.
};
