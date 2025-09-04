import { useEffect, useState } from 'react';
import HealthRecordService from '../services/health-record';
import { PatientService } from '../services/patient-service';
import { HealthRecord } from '../types/healthRecord';
import { Patient } from '../types/patient';

const usePatient = (patientId: string) => {
  const [patient, setPatient] = useState<Patient | null>(null);
  const [patientError, setPatientError] = useState<string>('');
  const [healthRecords, setHealthRecords] = useState<HealthRecord[]>([]);

  useEffect(() => {
    const fetchData = async () => {
      setPatientError('');
      try {
        const resPatient = await PatientService.getPatient(patientId);
        setPatient(resPatient);
        if (!resPatient) {
          setPatientError('Patient not found');
        }
        const resHealthRecords =
          await HealthRecordService.getHealthRecordsByPatientId(patientId);
        setHealthRecords(resHealthRecords);
      } catch (error) {
        console.error(error);
      }
    };
    fetchData();
  }, [patientId]);

  return { patient, healthRecords, patientError };
};

export default usePatient;
