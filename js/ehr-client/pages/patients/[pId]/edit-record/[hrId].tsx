import { useRouter } from 'next/router';
import { useEffect, useState } from 'react';
import InputArray from '../../../../src/form/inputArray';
import HealthRecordService from '../../../../src/services/health-record';
import { HealthRecord } from '../../../../src/types/healthRecord';

const EditRecord = () => {
  const router = useRouter();
  const { pId, hrId } = router.query;
  const [updatedHR, setUpdatedHR] = useState<HealthRecord>({
    id: '',
    patientId: `${pId}`,
    date: '',
    description: '',
    prescription: [],
    observations: []
  });
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    HealthRecordService.getHealthRecord(`${hrId}`, `${pId}`)
      .then((healthRecord) => {
        setError(null);
        if (healthRecord) setUpdatedHR(healthRecord);
        else throw new Error('Health record not found');
      })
      .catch((error) => {
        setError(error.message);
      });
  }, [pId, hrId]);

  if (error) return <div>{error}</div>;
  if (!updatedHR) return <div>Loading...</div>;

  const autoPropSetter = (prop: keyof HealthRecord) => ({
    value: updatedHR[prop],
    onChange: (
      e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>
    ) => {
      setUpdatedHR({ ...updatedHR, [prop]: e.target.value });
    }
  });

  const onSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const { date, description, patientId } = updatedHR;
    if (!date) {
      alert('Please enter a date');
      return;
    }
    if (!description) {
      alert('Please enter a description');
      return;
    }
    if (!patientId) {
      alert('Please enter a patientId');
      return;
    }
    try {
      await HealthRecordService.updateHealthRecord(updatedHR);
      router.push(`/patients/${pId}`);
    } catch (error) {
      alert(error);
    }
  };

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: '1rem',
        alignItems: 'center'
      }}
    >
      <h1>Edit Record</h1>
      <form
        onSubmit={onSubmit}
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '1rem',
          minWidth: '22rem'
        }}
      >
        <label>Date</label>
        <input
          type="date"
          {...autoPropSetter('date')}
          value={
            updatedHR.date
              ? new Date(updatedHR.date).toISOString().split('T')[0]
              : ''
          }
        />

        <label>Description</label>
        <textarea
          style={{
            width: '100%',
            height: '10rem',
            resize: 'vertical',
            overflow: 'auto',
            padding: '0.5rem',
            border: '1px solid #ccc',
            borderRadius: '0.25rem',
            boxSizing: 'border-box',
            fontSize: '1rem',
            fontFamily: 'sans-serif',
            lineHeight: '1.5',
            color: '#333',
            boxShadow: '0 1px 3px rgba(0,0,0,0.1)'
          }}
          {...autoPropSetter('description')}
        />

        <label>Prescription</label>
        <InputArray
          value={updatedHR.prescription}
          onChange={(newPrescription) => {
            setUpdatedHR({
              ...updatedHR,
              prescription: newPrescription
            });
          }}
        />

        <label>Observations</label>
        <InputArray
          value={updatedHR.observations}
          onChange={(newPbservations) => {
            setUpdatedHR({
              ...updatedHR,
              observations: newPbservations
            });
          }}
        />

        <button type="submit">Submit</button>
      </form>
      <style jsx>
        {`
          label {
            font-weight: bold;
            font-size: 1.2rem;
          }
          input {
            border: 1px solid #ccc;
            border-radius: 4px;
            padding: 0.5rem;
            width: 100%;
          }
          button {
            background-color: #00bcd4;
            color: #fff;
            border: none;
            padding: 0.5rem 1rem;
            border-radius: 0.25rem;
            cursor: pointer;
            height: 100%;
          }
        `}
      </style>
    </div>
  );
};
export default EditRecord;
