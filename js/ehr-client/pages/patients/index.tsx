import { NextPage } from 'next';
import Link from 'next/link';
import usePatientsPaginator from '../../src/hooks/usePatientsPaginator';
import { parseFormEvent } from '../../src/utils/fomr';

const Patients: NextPage = () => {
  const { patients, page, setPage, setSearch, totalPages } =
    usePatientsPaginator();

  const submitSearch = (e: React.FormEvent<HTMLFormElement>) => {
    const { search } = parseFormEvent<{ search: string }>(e);
    setSearch(search);
  };

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
        <h1>Patients</h1>
        <Link href="/patients/new">
          <a
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              padding: '0.5rem 1rem',
              borderRadius: '0.5rem',
              backgroundColor: '#00bcd4',
              color: '#fff'
            }}
          >
            New Patient
          </a>
        </Link>
      </div>
      <div>
        <form
          onSubmit={submitSearch}
          style={{
            display: 'flex',
            flexDirection: 'row',
            alignItems: 'center',
            gap: '1rem',
            marginBottom: '1rem'
          }}
        >
          <input
            style={{ margin: 0 }}
            type="text"
            name="search"
            placeholder="Search"
          />
          <button type="submit">Search</button>
        </form>

        <button onClick={() => setPage(1)} disabled={page === 1}>
          First
        </button>
        <button onClick={() => setPage(page - 1)} disabled={page === 1}>
          Previous
        </button>
        <button
          onClick={() => setPage(page + 1)}
          disabled={page === totalPages}
        >
          Next
        </button>
        <button
          onClick={() => setPage(totalPages)}
          disabled={page === totalPages}
        >
          Last
        </button>

        <p>
          Page {page} of {totalPages}
        </p>
      </div>
      <ul
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '1rem'
        }}
      >
        {patients.map((patient) => (
          <li
            key={`patient-${patient.id}`}
            style={{
              display: 'grid',
              gridTemplateColumns: '1fr 1fr auto',
              alignItems: 'center',
              gap: '1rem'
            }}
          >
            <p>{patient.ssn}</p>
            <p>{patient.names}</p>
            <Link href={`/patients/${patient.id}`}>
              <a
                style={{
                  display: 'flex',
                  flexDirection: 'row',
                  alignItems: 'center',
                  gap: '1rem',
                  cursor: 'pointer',
                  background: '#f0f0f0',
                  borderRadius: '0.5rem',
                  padding: '0.2rem 0.5rem',
                  border: '1px solid #ccc'
                }}
              >
                Open
              </a>
            </Link>
          </li>
        ))}
      </ul>
      <style jsx>{`
        input {
          width: 100%;
          padding: 0.5rem;
          border: 1px solid #ccc;
          border-radius: 4px;
        }
        button {
          padding: 0.5rem;
          border: 1px solid #ccc;
          border-radius: 4px;
          margin-right: 1rem;
        }
        ul {
          list-style: none;
          padding: 0;
          margin: 0;
        }
        li {
          padding: 0.5rem;
          border-bottom: 1px solid #ccc;
        }
      `}</style>
    </div>
  );
};

export default Patients;
