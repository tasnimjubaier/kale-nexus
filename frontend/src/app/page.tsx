'use client';
import { useState } from 'react';
import {
  rpc,
  Account,                // <-- add this
  TransactionBuilder,
  BASE_FEE,
  Networks,
  Operation,
} from '@stellar/stellar-sdk';
import {
  getRpc,
  connectWallet,
  signXdrWithXbull,
  submitSignedXdrAndWait,
} from '@/lib/wallet';

export default function Page() {
  const [pubkey, setPubkey] = useState<string>();

  const onConnect = async () => {
    const k = await connectWallet();
    setPubkey(k);
  };

  const onSignAndSend = async () => {
    if (!pubkey) return alert('Connect wallet first');

    const server = getRpc();

    // 1) Load RPC account, then wrap it as a Stellar Account
    const accResp = await server.getAccount(pubkey);   // Soroban RPC response
    const account = new Account(pubkey, accResp.sequenceNumber()); // <-- proper Account
    

    // 2) Build a tiny classic tx (no-op bumpSequence) to test signing/submitting
    const bumpTo = (BigInt(accResp.sequenceNumber()) + 1n).toString();
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: Networks.TESTNET, // guaranteed string
    })
      .addOperation(Operation.bumpSequence({ bumpTo }))
      .setTimeout(60)
      .build();

    // 3) Sign with xBull (must pass BOTH xdr + same passphrase)
    const signed = await signXdrWithXbull(tx.toXDR(), Networks.TESTNET, pubkey);

    // 4) Submit & wait for result
    const res = await submitSignedXdrAndWait(signed, Networks.TESTNET);
    alert(`Tx status: ${res.status}\nHash: ${res.txHash}`);
    console.log(res.txHash);
  };

  return (
    <main style={{ padding: 24 }}>
      <button onClick={onConnect}>Connect xBull</button>
      {pubkey && <p>Connected: {pubkey}</p>}
      <button onClick={onSignAndSend}>Sign & Send (test tx)</button>
    </main>
  );
}
