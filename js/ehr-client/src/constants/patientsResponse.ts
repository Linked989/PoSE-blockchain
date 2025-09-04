import { PatientResponse, PatientsResponse } from '../types/patient';

export const patientsResponse: PatientsResponse = {
  patients: [
    {
      id: '1',
      names: 'John',
      dateOfBirth: '01/01/2012',
      ssn: '123-45-6789',
    },
    {
      id: '2',
      names: 'Jane',
      dateOfBirth: '01/01/1820',
      ssn: '123-45-6789',
    },
    {
      id: '3',
      names: 'Bob',
      dateOfBirth: '01/01/1920',
      ssn: '123-45-6789',
    },
    {
      id: '4',
      names: 'Jerry',
      dateOfBirth: '01/01/2010',
      ssn: '123-45-6789',
    }
  ],
  totalPages: 1,
  currentPage: 1
};
