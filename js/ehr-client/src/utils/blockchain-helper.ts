import { ApiPromise, Keyring, WsProvider } from "@polkadot/api";
import { ContractPromise } from "@polkadot/api-contract";
import { mnemonicGenerate } from "@polkadot/util-crypto";
import metadata from '../../../../contract/health-record/build/metadata.json';
import { HealthRecord } from "../types/healthRecord";
import { Patient } from "../types/patient";


export const GeoblockchainConstants = {
    url: "ws://127.0.0.1:9944",
    contractId: "5Da53VsDbijcbAB1dPKZaruY44VpumNyww4PCpthi9MKTyHs",
    aliceId: "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
    gasLimit: -1,
    getPatients: async (apiPromise: ApiPromise | undefined = undefined) => {
        let patients = undefined;
        try {
            let apiInstance = apiPromise;
            if (!apiInstance || !apiInstance.isConnected) {
                const ws = new WsProvider(GeoblockchainConstants.url);
                apiInstance = await ApiPromise.create({ provider: ws });
            }

            const contract = new ContractPromise(apiInstance, metadata, GeoblockchainConstants.contractId);
            const { output } = await contract.query.getPatients(GeoblockchainConstants.aliceId, { gasLimit: GeoblockchainConstants.gasLimit });

            if (!apiPromise) {
                await apiInstance.disconnect();
            }

            patients = <Patient[]>output?.toHuman();
        } catch (error) {
            console.log(error);
        }

        return patients;
    },
    addPatient: async (patient: Patient) => {
        try {
            let keyring = new Keyring({ type: "sr25519" });
            const keyPair = keyring.addFromUri('//Alice');

            const ws = new WsProvider(GeoblockchainConstants.url);
            const apiInstance = await ApiPromise.create({ provider: ws });

            const contract = new ContractPromise(apiInstance, metadata, GeoblockchainConstants.contractId);
            await contract.tx["addPatient"]({ gasLimit: GeoblockchainConstants.gasLimit }, [patient.id, patient.names, patient.dateOfBirth, patient.ssn]).signAndSend(keyPair);
            await apiInstance.disconnect();
            return patient;

        } catch (error) {
            console.log(error);
            return undefined;
        }
    },
    updateHealthRecord: async (healthRecord: HealthRecord) => {
        try {
            let keyring = new Keyring({ type: "sr25519" });
            const keyPair = keyring.addFromUri('//Alice');

            const ws = new WsProvider(GeoblockchainConstants.url);
            const apiInstance = await ApiPromise.create({ provider: ws });

            const contract = new ContractPromise(apiInstance, metadata, GeoblockchainConstants.contractId);
            await contract.tx["updateHealthRecord"]({ gasLimit: GeoblockchainConstants.gasLimit }, [parseInt(healthRecord.id), parseInt(healthRecord.patientId), healthRecord.description, healthRecord.date]).signAndSend(keyPair);
            await apiInstance.disconnect();

            return healthRecord;
        } catch (error) {
            console.log(error);
            return undefined;
        }
    },
    addHealthRecord: async (healthRecord: HealthRecord) => {
        try {
            let keyring = new Keyring({ type: "sr25519" });
            const keyPair = keyring.addFromUri('//Alice');

            const ws = new WsProvider(GeoblockchainConstants.url);
            const apiInstance = await ApiPromise.create({ provider: ws });

            console.log(healthRecord);
            const contract = new ContractPromise(apiInstance, metadata, GeoblockchainConstants.contractId);
            await contract.tx["addHealthRecord"]({ gasLimit: GeoblockchainConstants.gasLimit }, [parseInt(healthRecord.id), parseInt(healthRecord.patientId), healthRecord.description, healthRecord.date]).signAndSend(keyPair);
            await apiInstance.disconnect();

            return healthRecord;

        } catch (error) {
            console.log(error);
            return undefined;
        }
    },
};