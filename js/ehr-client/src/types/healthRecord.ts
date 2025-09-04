export type HealthRecord = {
  id: string;
  patientId: string;
  description: string;
  observations: string[];
  date: string;
  prescription: string[];
};
