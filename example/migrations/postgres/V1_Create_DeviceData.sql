CREATE TABLE IF NOT EXISTS DeviceData
(
    id          SERIAL
        PRIMARY KEY,
    device_no   VARCHAR(36)    NULL,

    patientId   VARCHAR(36)    NULL,
    patientName VARCHAR(36)    NULL ,
    hisId                    VARCHAR(36)   NULL,
    departmentId             VARCHAR(255)  NULL,
    departmentName           VARCHAR(128)  NULL,
    SickbedNo   VARCHAR(63) NULL ,

    msh_time    BIGINT   NULL,
    msh_type    TEXT     NULL,
    vital_signs TEXT NULL,
    hl7         TEXT NULL
);

