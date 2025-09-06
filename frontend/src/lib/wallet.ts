'use client';

import { rpc, TransactionBuilder, Transaction, xdr } from '@stellar/stellar-sdk';
import {xBullWalletConnect} from '@creit.tech/xbull-wallet-connect';

let bridge: xBullWalletConnect | null = null;

/** Lazily create a bridge per session */
function getBridge() {
  if (typeof window === 'undefined') throw new Error('wallet must run in browser');
  if (!bridge) bridge = new xBullWalletConnect(); // prefers extension, falls back to webapp
  return bridge;
}

/** Connect xBull -> returns G... public key */
export async function connectWallet(): Promise<string> {
  const pubkey = await getBridge().connect(); // prompts extension or opens webapp
  return pubkey; // e.g., "GABCD..."
}

/** Build an RPC Server pointing to your Soroban RPC */
export function getRpc(): rpc.Server {
  const url = process.env.NEXT_PUBLIC_RPC_URL!;
  return new rpc.Server(url);
}

/**
 * Sign a prepared transaction XDR with xBull.
 * - xdrBase64: base64 TransactionEnvelope (from Transaction.toXDR())
 * - networkPassphrase: e.g., "Test SDF Network ; September 2015"
 * - optional: publicKey if you want to force a specific account
 */
export async function signXdrWithXbull(
  xdrBase64: string,
  networkPassphrase: string,
  publicKey?: string
): Promise<string> {
  const b = getBridge();
  const signedXdr = await b.sign({
    xdr: xdrBase64,
    ...(publicKey ? { publicKey } : {}),
    network: networkPassphrase,
  });
  return signedXdr; // base64 TransactionEnvelope with signature(s)
}

/**
 * Submit a signed XDR to Soroban RPC and wait for completion.
 * Returns the final RPC getTransaction response.
 */
export async function submitSignedXdrAndWait(
  signedXdr: string,
  networkPassphrase: string
) {
  const server = getRpc();

  // Re-hydrate the Transaction to extract hash easily
  const tx = TransactionBuilder.fromXDR(signedXdr, networkPassphrase) as Transaction;

  // Enqueue transaction
  const sendRes = await server.sendTransaction(tx);
  // sendRes.hash, sendRes.status: "PENDING"|"ERROR" etc.
  if (sendRes.status === 'ERROR') {
    throw new Error(`RPC sendTransaction error: ${JSON.stringify(sendRes)}`);
  }

  // Poll for completion
  let tries = 0;
  // eslint-disable-next-line no-constant-condition
  while (true) {
    const res = await server.getTransaction(sendRes.hash);
    if (res.status === 'SUCCESS' || res.status === 'FAILED') return res;
    await new Promise(r => setTimeout(r, 1500));
    if (++tries > 40) throw new Error('Timed out waiting for transaction result');
  }
}

/** Clean up listeners when you no longer need the bridge (e.g., on logout) */
export function disconnectWallet() {
  try {
    bridge?.closeConnections(); // important per xBull docs
  } finally {
    bridge = null;
  }
}
