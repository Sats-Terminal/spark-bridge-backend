const RPC_USER = 'bitcoin';
const RPC_PASS = 'bitcoinpass';
const RPC_URL = 'http://127.0.0.1:18443/';

export function call_rpc(method: string, params: any[] = []): any {
  return fetch(RPC_URL, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization:
        'Basic ' + Buffer.from(`${RPC_USER}:${RPC_PASS}`).toString('base64'),
    },
    body: JSON.stringify({ jsonrpc: '2.0', id: '0', method, params }),
  }).then(async (r) => {
    const data: any = await r.json();
    if (data.error) throw new Error(JSON.stringify(data.error));
    return data.result;
  });
}
