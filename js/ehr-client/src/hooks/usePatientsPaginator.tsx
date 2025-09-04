import { useEffect, useState } from 'react';
import { PatientService } from '../services/patient-service';
import { Patient } from '../types/patient';

const LIMIT = 5;

const usePatientsPaginator = () => {
  const [patients, setPatients] = useState<Patient[]>([]);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<any>('');
  const [search, setSearch] = useState('');

  const fetchPatients = async (page: number, search: string) => {
    setLoading(true);
    try {
      const data = await PatientService.getPatients(page, search, LIMIT);
      setPatients(data.patients);
      setTotalPages(data.totalPages);
    } catch (error) {
      setError(error);
    }
    setLoading(false);
  };

  useEffect(() => {
    fetchPatients(page, search);
  }, [page, search]);

  return {
    patients,
    page,
    totalPages,
    loading,
    error,
    setPage,
    search,
    setSearch
  };
};

export default usePatientsPaginator;
