CREATE TABLE IF NOT EXISTS VitalSign
(
    id          SERIAL
    PRIMARY KEY,
    patientId   VARCHAR(36)    NULL,
    vital_sign_name   VARCHAR(36)    NULL,
    vital_sign_value  VARCHAR(36) NULL ,
    vital_sign_unit  VARCHAR(36) NULL ,
    acq_timestamp BIGINT   NULL,
    time_slot BIGINT NULL,
    record_timestamp BIGINT   NULL,
    userId VARCHAR(36) NULL

    );


