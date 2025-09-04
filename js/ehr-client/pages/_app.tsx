import type { AppProps } from 'next/app';
import useDoctor from '../src/hooks/useDoctor';
import { useRouter } from 'next/router';
import Link from 'next/link';
import '../styles/globals.css';

const EHRLayout = ({ Component, pageProps }: AppProps) => {
  const router = useRouter();
  const { doctor, loading: loadingDoctor, logOut } = useDoctor();

  if (loadingDoctor) {
    return <div>Loading...</div>;
  }

  return (
    <div className="container">
      <header
        style={{
          display: 'flex',
          gap: '0.5rem',
          alignItems: 'center',
          alignSelf: 'end',
          borderBottom: '1px solid #ccc',
          width: '100%',
          justifyContent: 'space-between',
          padding: '0.5rem 1.5rem'
        }}
      >
        <div
          style={{
            display: 'flex',
            gap: '1rem'
          }}
        >
          <div
            style={{
              display: 'flex',
              alignItems: 'flex-end',
              fontSize: '1rem',
              fontWeight: 'bold',
              color: '#333',
              textDecoration: 'none',
              border: '0',
              borderRadius: '4px',
              backgroundColor: '#fff'
            }}
          >
            EHR System
          </div>
          {doctor && (
            <Link href="/patients">
              <button className="patients-link">Patients</button>
            </Link>
          )}
        </div>
        {doctor && (
          <div
            style={{
              display: 'flex',
              gap: '0.5rem',
              alignItems: 'center'
            }}
          >
            <p>
              Welcome
              <span style={{ color: '#0070f3' }}> {doctor.name}</span>
            </p>
            <button
              style={{
                backgroundColor: '#0070f3',
                color: '#fff',
                border: 'none',
                borderRadius: '4px',
                padding: '0.5rem',
                cursor: 'pointer'
              }}
              onClick={() => {
                logOut().then(() => router.push('/login'));
              }}
            >
              Logout
            </button>
          </div>
        )}
      </header>
      <div
        style={{
          margin: '1rem 2rem 2rem 2rem'
        }}
      >
        <Component {...pageProps} />
      </div>
      <style jsx>{`
        .patients-link {
          display: flex;
          align-items: flex-end;
          font-size: 1rem;
          font-weight: normal;
          color: #333;
          cursor: pointer;
          border: 0;
          border-radius: 4px;
          background-color: #fff;
        }
        .patients-link:hover {
          text-decoration: underline;
        }
      `}</style>
    </div>
  );
};

function MyApp(appProps: AppProps) {
  return <EHRLayout {...appProps} />;
}

export default MyApp;
