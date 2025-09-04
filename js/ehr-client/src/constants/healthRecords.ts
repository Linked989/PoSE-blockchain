import { HealthRecord } from '../types/healthRecord';

export const records: HealthRecord[] = [
  {
    id: '123',
    description: 'Fever',
    observations: ['High Fever', 'Cough'],
    date: new Date(Date.now() - 1000 * 60 * 60 * 24 * 365).toISOString(),
    patientId: 'x',
    prescription: ['Tylenol']
  },
  {
    id: '456',
    description: 'Flu',
    observations: ['Flu'],
    date: new Date(Date.now() - 1000 * 60 * 60 * 24 * 365).toISOString(),
    patientId: 'x',
    prescription: ['Ventolin']
  },
  {
    id: '123223',
    description: 'Fever',
    observations: ['High Fever', 'Cough'],
    date: new Date(Date.now() - 1000 * 60 * 60 * 24 * 365).toISOString(),
    patientId: 'x',
    prescription: ['Tylenol']
  }
];
