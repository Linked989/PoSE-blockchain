import { patientsResponse } from '../constants/patientsResponse';
import { Patient } from '../types/patient';
import { uniqueNamesGenerator, names } from 'unique-names-generator';
import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';
const { encodeAddress, mnemonicGenerate } = require('@polkadot/util-crypto');
import { GeoblockchainConstants } from '../utils/blockchain-helper';
import { ContractPromise } from '@polkadot/api-contract';


const getLocalStoragePatients = (
  page: number,
  search: string,
  limit: number
): { patients: Patient[]; totalPages: number } => {
  const LSPatients = localStorage.getItem('patients');
  if (LSPatients) {
    const patients = JSON.parse(LSPatients);
    const filteredPatients = patients.filter((patient: Patient) =>
      patient.names.toLowerCase().includes(search.toLowerCase())
    );
    const paginatedPatients = filteredPatients.slice(
      (page - 1) * limit,
      (page - 1) * limit + limit
    );
    return {
      patients: paginatedPatients,
      totalPages: Math.ceil(filteredPatients.length / limit)
    };
  }

  return { patients: [], totalPages: 0 };
  // } else {
  //   localStorage.setItem('patients', JSON.stringify(patientsResponse.patients));
  //   return getLocalStoragePatients(page, search, limit);
  // }
};

export const PatientService = {
  getPatients: async (page: number, search: string, limit: number) => {
    const localPatients = getLocalStoragePatients(page, search, limit);

    if (!localPatients || localPatients.totalPages <= 0) {
      try {
        // const ws = new WsProvider(GeoblockchainConstants.url);
        // const apiPromise = await ApiPromise.create({ provider: ws });
        // // const patients = (await apiPromise.query.system.account.entries()).map<Patient>((x) => (<Patient>{ id:encodeAddress(x[0].slice(-32)) , name: uniqueNamesGenerator({ dictionaries: [names] }) }));
        // // const patients = (await apiPromise.query.system.account.entries()).map<Patient>((x) => (<Patient>{ id:Math.random().toString(10).substring(2, 5) ,key: encodeAddress(x[0].slice(-32)), name: uniqueNamesGenerator({ dictionaries: [names] }) }));
        // const entries = await apiPromise.query.system.account.entries();

        const patients: Patient[] = (await GeoblockchainConstants.getPatients())!;

        // for (let index = 0; index < entries.length; index++) {
        //   let patient = undefined;

        //   try {
        //     const accountId = encodeAddress(entries[index][0].slice(-32));
        //     patient = await GeoblockchainConstants.getPatient(accountId, apiPromise);
        //   } catch (error) {
        //   }

        //   if (patient) {
        //     patients.push(patient);
        //   }

        // }

        localStorage.setItem('patients', JSON.stringify(patients));
        // apiPromise.disconnect();
      } catch (error) {
        console.log(error);
      }
    }

    return getLocalStoragePatients(page, search, limit);
  },
  createPatient: async (patientWithoutId: Omit<Patient, 'id'>) => {

    const LSPatients = localStorage.getItem('patients');
    if (LSPatients) {

      const patients: Patient[] = JSON.parse(LSPatients);
      const patient = await GeoblockchainConstants.addPatient({ ...patientWithoutId, id: (patients?.at(-1)?.id!) + 1 });

      if (patient) {
        patients.push(patient);
        localStorage.setItem('patients', JSON.stringify(patients));
      }

    }
  },
  getPatient: async (id: string) => {
    const LSPatients = localStorage.getItem('patients');
    if (LSPatients) {
      const patients = JSON.parse(LSPatients);
      const patient: Patient = patients.find(
        (patient: Patient) => patient.id === id
      );
      return patient;
    } else {
      return null;
    }
  }
};
