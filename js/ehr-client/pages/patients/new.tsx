import { NextPage } from 'next';
import { useRouter } from 'next/router';
import { useState } from 'react';
import { PatientService } from '../../src/services/patient-service';
import { Patient } from '../../src/types/patient';

const NewPatient: NextPage = () => {
  const router = useRouter();
  const [newPatient, setNewPatient] = useState<Patient>({
    names: '',
    dateOfBirth: '',
    id: '',
    ssn: ''
  });

  const autoPropSetter = (prop: keyof Patient) => ({
    value: newPatient[prop],
    onChange: (e: React.ChangeEvent<HTMLInputElement>) => {
      setNewPatient({ ...newPatient, [prop]: e.target.value });
    }
  });

  const onSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const { dateOfBirth, names: name, ssn } = newPatient;
    if (!dateOfBirth || dateOfBirth > new Date().toISOString()) {
      alert('Date of birth must be in the past');
      return;
    }
    if (!name) {
      alert('Name is required');
      return;
    }
    if (!ssn) {
      alert('SSN is required');
      return;
    }
    try {
      await PatientService.createPatient(newPatient);
      router.push('/patients');
    } catch (error) {
      alert(error);
    }
  };

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center'
      }}
    >
      <h1>New Patient</h1>
      <form
        onSubmit={onSubmit}
        style={{
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          gap: '1rem',
          marginTop: '1rem'
        }}
      >
        <label>
          Name:
          <input type="text" {...autoPropSetter('names')} />
        </label>
        <label>
          Date of Birth:
          <input type="date" {...autoPropSetter('dateOfBirth')} />
        </label>
        <label>
          SSN:
          <input type="text" {...autoPropSetter('ssn')} />
        </label>
        <input type="submit" value="Create" />
      </form>
      <style jsx>
        {`
          form {
            display: flex;
          }
          label {
            display: flex;
            flex-direction: column;
            align-items: center;
            gap: 1rem;
          }
          input {
            width: 20rem;
            height: 2rem;
            border-radius: 0.5rem;
            border: 1px solid #ccc;
            padding: 0.5rem;
          }
          input[type='submit'] {
            background-color: #00bcd4;
            color: #fff;
            border: none;
            border-radius: 0.5rem;
            padding: 0.5rem;
            cursor: pointer;
            margin-top: 1rem;
          }
        `}
      </style>
    </div>
  );
};

export default NewPatient;
