import { useRouter } from 'next/router';
import { useState } from 'react';
import InputArray from '../../../src/form/inputArray';
import HealthRecordService from '../../../src/services/health-record';
import { HealthRecord } from '../../../src/types/healthRecord';

const NewRecord = () => {
  const router = useRouter();
  const { pId } = router.query;
  const [newHealthRecord, setNewHealthRecord] = useState<HealthRecord>({
    id: '',
    patientId: `${pId}`,
    date: '',
    description: '',
    prescription: [],
    observations: []
  });

  const autoPropSetter = (prop: keyof HealthRecord) => ({
    value: newHealthRecord[prop],
    onChange: (
      e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>
    ) => {
      setNewHealthRecord({ ...newHealthRecord, [prop]: e.target.value });
    }
  });

  const onSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const { date, description, patientId } = newHealthRecord;
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
      await HealthRecordService.createHealthRecord(newHealthRecord);
      router.push(`/patients/${pId}`);
    } catch (error) {
      alert(error);
    }
  };

  const createTestHR = async () => {
    try {
      await HealthRecordService.createTestRecords(`${pId}`);
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
      <button
        onClick={(e) => {
          e.preventDefault();
          createTestHR();
        }}
      >
        Create 3 Health Records for testing
      </button>
      <h1>New Record</h1>
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
            newHealthRecord.date
              ? new Date(newHealthRecord.date).toISOString().split('T')[0]
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
          value={newHealthRecord.prescription}
          onChange={(newPrescription) => {
            setNewHealthRecord({
              ...newHealthRecord,
              prescription: newPrescription
            });
          }}
        />

        <label>Observations</label>
        <InputArray
          value={newHealthRecord.observations}
          onChange={(newPbservations) => {
            setNewHealthRecord({
              ...newHealthRecord,
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
export default NewRecord;
