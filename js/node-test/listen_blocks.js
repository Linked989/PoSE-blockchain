const { WsProvider, ApiPromise, Keyring } = require('@polkadot/api');
const { ContractPromise } = require('@polkadot/api-contract');
const { encodeAddress, mnemonicGenerate, cryptoWaitReady } = require('@polkadot/util-crypto');
const wsUrl = 'ws://localhost:9944';

(async () => {

    await cryptoWaitReady();

    console.log("CONEJO1");
    const provider = new WsProvider(wsUrl);
    console.log("CONEJO2");
    const api = await ApiPromise.create({provider : provider});
    console.log("CONEJO3");

    const unsub = await api.derive.chain.subscribeNewHeads((lastHeader) => {
        console.log(`#${lastHeader.number} was authored by ${lastHeader.author}`);
    });
    // api.disconnect();

})();
