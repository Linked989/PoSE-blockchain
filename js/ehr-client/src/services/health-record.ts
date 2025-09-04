import { records } from '../constants/healthRecords';
import { HealthRecord } from '../types/healthRecord';
import { ApiPromise, Keyring, WsProvider } from '@polkadot/api'
import { ContractPromise } from '@polkadot/api-contract';
import metadata from '../../../../contract/health-record/build/metadata.json';
import { GeoblockchainConstants } from '../utils/blockchain-helper';


const getLocalStorageHealthRecords = (patientId: string): HealthRecord[] => {
  const LSHealthRecords = localStorage.getItem('healthRecords');
  if (LSHealthRecords) {
    const healthRecords = JSON.parse(LSHealthRecords);
    const patientHealthRecords: HealthRecord[] = healthRecords.filter(
      (healthRecord: HealthRecord) => healthRecord.patientId === patientId
    );
    return patientHealthRecords;
  }

  return [];
  // } else {
  //   localStorage.setItem('healthRecords', JSON.stringify([]));
  //   return getLocalStorageHealthRecords(patientId);
  // }
};

const HealthRecordService = {
  getHealthRecordsByPatientId: async (patientId: string) => {
    const localHealthRecord = getLocalStorageHealthRecords(patientId);

    if (!localHealthRecord || localHealthRecord.length <= 0) {
      try {
        const ws = new WsProvider(GeoblockchainConstants.url);
        const apiPromise = await ApiPromise.create({ provider: ws });
        const contract = new ContractPromise(apiPromise, metadata, GeoblockchainConstants.contractId);
        const { output } = await contract.query.getHealthRecords(GeoblockchainConstants.aliceId, { gasLimit: GeoblockchainConstants.gasLimit });
        // console.log(result.toHuman());
        console.log(output?.toHuman());
        let healthRecords: HealthRecord[] = <HealthRecord[]>output?.toHuman();
        healthRecords = healthRecords.filter(x => x.patientId == patientId);

        const allLocalHealthRecords =  localStorage.getItem('healthRecords');
        let allHealthRecords: HealthRecord[] = [];
        if(allLocalHealthRecords){
          allHealthRecords = JSON.parse(allLocalHealthRecords);
        }

        localStorage.setItem('healthRecords', JSON.stringify( allHealthRecords && allHealthRecords.length > 0? allHealthRecords.concat(healthRecords.filter(x => x.patientId == patientId).map((x) => ({ ...x, prescription: [], observations: [] }))): healthRecords.map((x) => ({ ...x, prescription: [], observations: [] }) )));
        // await HealthRecordService.createHealthRecord({ ...<HealthRecord>output?.toHuman(), prescription: [], observations: [], patientId });
        apiPromise.disconnect();
      } catch (error) {
        console.log(error);
      }
    }

    return getLocalStorageHealthRecords(patientId);
    // return getLocalStorageHealthRecords(patientId);
  },
  createHealthRecord: async (healthRecord: HealthRecord) => {
    const LSHealthRecords = localStorage.getItem('healthRecords');
    if (LSHealthRecords) {
      const healthRecords: HealthRecord[] = JSON.parse(LSHealthRecords);

      let id = 1;
      if (healthRecords.length > 0) {
        id = parseInt((healthRecords?.at(-1)?.id!)) + 1;
      }

      healthRecord = (await GeoblockchainConstants.addHealthRecord({ ...healthRecord, id: id.toString() }))!;

      if (healthRecord) {
        healthRecords.push(healthRecord);
        localStorage.setItem('healthRecords', JSON.stringify(healthRecords));
      }

    }
  },
  updateHealthRecord: async (healthRecord: HealthRecord) => {
    const LSHealthRecords = localStorage.getItem('healthRecords');
    const healthRecordUpdated = await GeoblockchainConstants.updateHealthRecord(healthRecord);
    if (LSHealthRecords && healthRecordUpdated) {
      const healthRecords = JSON.parse(LSHealthRecords);
      const index = healthRecords.findIndex(
        (healthRecordItem: HealthRecord) =>
          healthRecordItem.id === healthRecordUpdated.id
      );
      healthRecords[index] = healthRecord;
      localStorage.setItem('healthRecords', JSON.stringify(healthRecords));
    }
  },
  getHealthRecord: async (healthRecordId: string, patientId: string) => {
    const LSHealthRecords = localStorage.getItem('healthRecords');
    if (LSHealthRecords) {
      const healthRecords = JSON.parse(LSHealthRecords);
      const healthRecord = healthRecords.find(
        (healthRecord: HealthRecord) =>
          healthRecord.id === healthRecordId &&
          healthRecord.patientId === patientId
      ) as HealthRecord;
      if (healthRecord) return healthRecord as HealthRecord;
    }
  },
  createTestRecords: async (patientId: string) => {
    function randomDate(start: Date, end: Date) {
      return new Date(
        start.getTime() + Math.random() * (end.getTime() - start.getTime())
      );
    }

    const newHealthRecords = records.map((record: HealthRecord) => ({
      ...record,
      patientId,
      id: Math.random().toString(16).substring(2, 8),
      date: randomDate(new Date(2000, 0, 1), new Date()).toISOString()
    }));
    const LSHealthRecords = localStorage.getItem('healthRecords');
    if (LSHealthRecords) {
      const healthRecords: HealthRecord[] = JSON.parse(LSHealthRecords);
      newHealthRecords.forEach((healthRecord: HealthRecord) => {
        healthRecords.push(healthRecord);
      });
      localStorage.setItem('healthRecords', JSON.stringify(healthRecords));
    }
  }
};

export default HealthRecordService;
