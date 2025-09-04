import { useRouter } from 'next/router';
import { useEffect } from 'react';
import type { NextPage } from 'next';
import Link from 'next/link';

const Home: NextPage = () => {
  const router = useRouter();

  useEffect(() => {
    router.push('/login');
  }, []);

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        gap: '1rem'
      }}
    >
      <h1>Home</h1>

      <button>
        <Link href="/login">
          <a>Login</a>
        </Link>
      </button>

      <style jsx>{`
        h1 {
          text-align: center;
        }
        button {
          background-color: #ccc;
          color: #fff;
          border: none;
          border-radius: 0.5rem;
          padding: 0.5rem;
          cursor: pointer;
        }
        button:hover {
          background-color: #0070f3;
        }
      `}</style>
    </div>
  );
};

export default Home;
