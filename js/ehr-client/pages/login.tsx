import { NextPage } from 'next';
import { useRouter } from 'next/router';
import { useState } from 'react';
import useDoctor from '../src/hooks/useDoctor';

const Login: NextPage = () => {
  const router = useRouter();
  const { logIn } = useDoctor();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');

  const onSubmit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (email === 'user' && password === 'qwerty1234') {
      logIn().then(() => router.push('/patients').then(() => router.reload()));
    } else {
      setError('Invalid email or password');
      setTimeout(() => setError(''), 3000);
    }
  };

  return (
    <div>
      <h1>Login</h1>

      <form onSubmit={onSubmit}>
        <label>
          Email:
          <input
            type="text"
            name="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
          />
        </label>
        <label>
          Password:
          <input
            type="password"
            name="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </label>
        {error && (
          <div
            style={{
              color: 'red',
              fontSize: '0.8rem',
              marginTop: '0.5rem'
            }}
          >
            <p>{error}</p>
          </div>
        )}
        <input type="submit" value="Login" />
      </form>
      <style jsx>{`
        form {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 1rem;
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
          background-color: #ccc;
          color: #fff;
          border: none;
          border-radius: 0.5rem;
          padding: 0.5rem;
          cursor: pointer;
        }
      `}</style>
    </div>
  );
};

export default Login;
