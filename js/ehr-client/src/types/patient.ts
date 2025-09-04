export type Patient = {
  id: string;
  names: string;
  dateOfBirth: string;
  ssn: string;
};

export type PatientsResponse = {
  patients: Patient[];
  totalPages: number;
  currentPage: number;
};

export type PatientResponse = {
  patient: Patient;
};
