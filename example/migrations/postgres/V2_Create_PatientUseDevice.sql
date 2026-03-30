CREATE TABLE IF NOT EXISTS PatientUseDevice
(
    id          SERIAL PRIMARY KEY,
    device_no   VARCHAR(36)    NULL,
    patientId   VARCHAR(36)    NULL,
    hisId       VARCHAR(36)   NULL,
    start_time  BIGINT   NULL,
    end_time    BIGINT NULL
    );
