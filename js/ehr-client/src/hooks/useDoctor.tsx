import { useEffect, useState } from 'react';
import { Doctor } from '../types/doctor';

const useDoctor = () => {
  const [doctor, setDoctor] = useState<Doctor | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<any>('');

  const fetchData = async () => {
    try {
      setLoading(true);
      //   const response = await fetch('/api/doctor');
      //   const data = await response.json();
      const data = {
        id: 1,
        name: 'Dr. John Doe',
        image: 'https://placeimg.com/200/200/people',
        description: 'Lorem ipsum dolor sit amet, consectetur adipiscing elit,',
        specialization: 'General Practitioner'
      };
      setDoctor(data);
    } catch (error) {
      setError(error);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchData();
  }, []);

  const logOut = async () => {
    setDoctor(null);
  };

  const logIn = async () => {
    return fetchData();
  };

  return { doctor, loading, error, logOut, logIn };
};

export default useDoctor;
