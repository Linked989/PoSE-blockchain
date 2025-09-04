import { FC, useRef, useState } from 'react';

type InputArrayProps = {
  value: string[];
  onChange: (v: string[]) => void;
};

const InputArray: FC<InputArrayProps> = ({ value, onChange }) => {
  const [inputValue, setInputValue] = useState('');
  const inputValueRef = useRef<HTMLInputElement>(null);

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
          display: 'grid',
          gridTemplateColumns: '1fr auto',
          gap: '1rem',
          alignItems: 'center',
          justifyContent: 'space-between'
        }}
      >
        <input
          ref={inputValueRef}
          style={{
            width: '100%',
            border: '1px solid #ccc',
            borderRadius: '4px',
            padding: '0.5rem'
          }}
          type="text"
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
        />
        <button
          style={{
            backgroundColor: '#00bcd4',
            color: '#fff',
            border: 'none',
            padding: '0.5rem 1rem',
            borderRadius: '0.25rem',
            cursor: 'pointer',
            height: '100%'
          }}
          onClick={(e) => {
            e.preventDefault();
            if (inputValue) {
              if (value.includes(inputValue)) {
                alert('This value already exists');
              } else {
                onChange([...value, inputValue]);
                setInputValue('');
              }
            }
            inputValueRef?.current?.focus();
          }}
        >
          Add
        </button>
      </div>
      <ul
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: '1rem'
        }}
      >
        {value.map((v, i) => (
          <li
            key={i}
            style={{
              display: 'flex',
              flexDirection: 'row',
              gap: '1rem',
              alignItems: 'center',
              justifyContent: 'space-between'
            }}
          >
            - {v}
            <button
              style={{
                backgroundColor: '#ffffff',
                color: '#d71818',
                border: 'none',
                borderRadius: '0.25rem',
                cursor: 'pointer'
              }}
              onClick={(e) => {
                e.preventDefault();
                onChange(value.filter((_, j) => j !== i));
              }}
            >
              Remove
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
};

export default InputArray;
