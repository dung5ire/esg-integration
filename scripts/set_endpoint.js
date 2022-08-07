const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');
require("dotenv").config()

console.log(process.env.COMPANY_ENDPOINT)

const company = process.env.COMPANY_NAME

const endpoint = process.env.COMPANY_ENDPOINT
const main = async() => {
  const provider = new WsProvider('ws://127.0.0.1:9944');
  //const provider = new HttpProvider('http://localhost:9933');
  const api = await ApiPromise.create({ provider });
  const keyring = new Keyring({ type: 'sr25519' });

  // TEST ACCOUNT
  const ALICE = keyring.addFromUri("//Alice");

    const unsub = await api.tx.esgFetcher
  .setUrl(endpoint + company)
  .signAndSend(ALICE, (result) => {
    console.log(`Current status is ${result.status}`);

  if (result.status.isFinalized) {
      console.log(`Transaction finalized at blockHash ${result.status.asFinalized}`);
      console.log("Set url successfully");
      unsub();
    }
  });


}
// main().catch(console.error).finally(() => process.exit());

main();