import { NextPage } from 'next';
import Link from 'next/link';
import { useRouter } from 'next/router';
import usePatient from '../../src/hooks/usePatient';

const PatientPage: NextPage = () => {
  const router = useRouter();
  const { pId } = router.query;
  const { patient, healthRecords, patientError } = usePatient(
    pId?.toString() ?? ''
  );

  if (!pId) return <div>No patient id</div>;
  if (patientError) return <div>{patientError}</div>;
  if (!patient) return <div>Loading...</div>;
  if (!healthRecords) return <div>Loading...</div>;

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: '1rem'
      }}
    >
      <div
        style={{
          display: 'flex',
          flexDirection: 'row',
          gap: '1rem',
          alignItems: 'center',
          justifyContent: 'space-between'
        }}
      >
        <h1>Patient Page</h1>
        <Link href={`/patients/${pId}/new-record`}>
          <a
            style={{
              display: 'flex',
              padding: '0.5rem',
              height: 'auto',
              borderRadius: '0.5rem',
              backgroundColor: '#00bcd4',
              color: '#fff'
            }}
          >
            New Record
          </a>
        </Link>
      </div>
      <p>
        <strong>Name:</strong> {patient.names}
      </p>
      <p>
        <strong>Health Records:</strong>
      </p>
      <ul
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '1rem'
        }}
      >
        {healthRecords
          .sort((a, b) => {
            if (a.date < b.date) return 1;
            if (a.date > b.date) return -1;
            return 0;
          })
          .map((healthRecord) => (
            <li
              key={healthRecord.id}
              style={{
                display: 'grid',
                gridTemplateColumns: 'auto 1fr 1fr 1fr 1fr',
                gap: '1.5rem',
                borderBottom: '1px solid #ccc',
                padding: '1.5rem 1rem'
              }}
            >
              <Link href={`/patients/${pId}/edit-record/${healthRecord.id}`}>
                <a
                  style={{
                    display: 'flex',
                    padding: '0.3rem 0.5rem',
                    height: 'min-content',
                    borderRadius: '0.5rem',
                    backgroundColor: '#00bcd4',
                    color: '#fff'
                  }}
                >
                  Edit
                </a>
              </Link>
              <p>
                <strong>Date:</strong>{' '}
                {new Date(healthRecord.date).toLocaleDateString()}
              </p>
              <p>
                <strong>Description:</strong>{' '}
                {healthRecord.description?.length > 50
                  ? healthRecord.description.slice(0, 50).concat('...')
                  : healthRecord.description}
              </p>
              <p>
                <strong>Prescription:</strong>{' '}
                {healthRecord.prescription?.join(', ')}
              </p>
              <p>
                <strong>Observations:</strong>{' '}
                {healthRecord.observations?.join(', ')}
              </p>
            </li>
          ))}
      </ul>
      <style jsx>
        {`
          h1 {
            font-size: 2rem;
            font-weight: bold;
          }
          p {
            font-size: 1.2rem;
          }
          ul {
            list-style: none;
            padding: 0;
          }
        `}
      </style>
    </div>
  );
};

export default PatientPage;
