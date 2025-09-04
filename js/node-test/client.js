const { WsProvider, ApiPromise, Keyring } = require('@polkadot/api');
const { ContractPromise } = require('@polkadot/api-contract');
const { encodeAddress, mnemonicGenerate, cryptoWaitReady } = require('@polkadot/util-crypto');
const wsUrl = 'ws://localhost:9944';

//make sure this is the correct address for //Alice.
//for now, on the command line, do:
//
//   substrate-contracts-node key inspect //Alice
//
//you should see the address:
//
//  Public key (SS58): 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
//  SS58 Address:      5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
//

//Keyring initialize
let keyring = new Keyring({ type: "sr25519" });

const mnemonic = mnemonicGenerate(12);
console.log(mnemonic);

//Hardcoded default alice addres
const aliceAddress = '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY';

//make sure this path is correct for YOUR project.
const metadataPath = '../../contract/health-record/build/metadata.json';

//convert the metadata file constants into local variable
const metadata = require(metadataPath);

//make sure this address is correct for YOUR uploaded/instantiated contract.
const contractAddress = '5EY5X3kj6c8NfykGNSuhwoKcynCpD5mEFzWgfeehL7Mb7yAK';

(async () => {

    //wait to initialize blockchain
    await cryptoWaitReady();

    const accTest = keyring.addFromUri(mnemonic, { name: 'mnemonic acc' }, 'sr25519');

    //connect to our local substrate node
    const ws = new WsProvider(wsUrl);

    const wsApi = await ApiPromise.create({ provider: ws });
    const contract = new ContractPromise(wsApi, metadata, contractAddress);

    console.log((await wsApi.query.system.account.entries()).map((x) => encodeAddress(x[0].slice(-32))));
    console.log((await wsApi.query.system.account.entries())[0]);
    // console.log('query:\n', contract.query);
    // console.log('tx:\n', contract.tx);

    await wsApi.tx.sudo.sudo(
        wsApi.tx.balances.setBalance(accTest.address,'11529','0')
    ).signAndSend(aliceKeypair, (result) => { console.log(result); });

    let aliceKeypair = keyring.addFromUri('//Alice');


    const gasLimit = -1;
    const storageDepositLimit = undefined;


    let conejo = await contract.query.get(accTest.address, { gasLimit, storageDepositLimit });
    console.log(conejo.output?.toHuman());

    // TRANSACTION - MODIFICAR VALORES
    await contract.tx["flip"]({ gasLimit, storageDepositLimit }).signAndSend(aliceKeypair, async result => {
        console.log(result);
        result.events.forEach(record => {
            const { event } = record;
            const key = `${event.section}:${event.method}`;
            console.log(key);
        });
    });

    const values = [2,2,"conejo2", "2022-09-28"];
    await contract.tx["addHealthRecord"]({ gasLimit, storageDepositLimit }, values).signAndSend(aliceKeypair);

    console.log(conejo);





    // const conejo = await contract.query["flip"]({ gasLimit, storageDepositLimit });

    // conejo = await contract.query.get(aliceAddress, { gasLimit, storageDepositLimit });
    // console.log(conejo.output?.toHuman());


    // console.log(conejo.toHuman());

    // console.log(healthRecord[patientId]);

    // console.log(contract.registry.findMetaError(conejo.result.asErr.asModule));
    // console.log(conejo?.result.toHuman());
    // console.log(conejo?.output?.toHuman());

    // console.log('result:', result.toHuman());
    // console.log('out:', output != undefined ? output.toHuman() : null);

    wsApi.disconnect();
})();