// Migrations are an early feature. Currently, they're nothing more than this
// single deploy script that's invoked from the CLI, injecting a provider
// configured from the workspace's Anchor.toml.

const anchor = require("@project-serum/anchor");
const { TOKEN_PROGRAM_ID } = require("@solana/spl-token");
const fs = require("fs");

module.exports = async function (provider) {
  // Configure client to use the provider.
  anchor.setProvider(provider);

  let program = anchor.workspace.StepStaking;

  let mintPubkey = new anchor.web3.PublicKey(
    "AURYydfxJib1ZkTir1Jn1J9ECYUtjb6rKQVmtYaixWPP"
  );

  const [vaultPubkey, vaultBump] =
    await anchor.web3.PublicKey.findProgramAddress(
      [mintPubkey.toBuffer()],
      program.programId
    );

  const [stakingPubkey, stakingBump] =
    await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from(anchor.utils.bytes.utf8.encode("staking"))],
      program.programId
    );
  console.log(vaultPubkey.toString(), vaultBump);
  console.log(stakingPubkey.toString(), stakingBump);

  const lockEndDate = new anchor.BN("1653577200");

  await program.rpc.initialize(vaultBump, stakingBump, lockEndDate, {
    accounts: {
      tokenMint: mintPubkey,
      tokenVault: vaultPubkey,
      stakingAccount: stakingPubkey,
      initializer: provider.wallet.publicKey,
      systemProgram: anchor.web3.SystemProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID,
      rent: anchor.web3.SYSVAR_RENT_PUBKEY,
    },
  });
};
