import React from 'react';

const MyComponent = ({ title }) => {
  return (
    <div>
      <h1>{title}</h1>
    </div>
  );
};

export default MyComponent;

export function HeaderComponent() {
  return <header>Header</header>;
}

export const normalFunction = () => {
  return "Not a component";
};
