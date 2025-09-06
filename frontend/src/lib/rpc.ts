import { rpc } from "@stellar/stellar-sdk";
const server = new rpc.Server(process.env.NEXT_PUBLIC_RPC_URL!);
const net = await server.getNetwork();
console.log(net);