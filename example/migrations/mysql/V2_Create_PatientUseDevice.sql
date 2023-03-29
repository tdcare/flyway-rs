create table if not exists PatientUseDevice
(
    id          int auto_increment primary key,
    device_no   varchar(36)    null,
    patientId   varchar(36)    null,
    hisId       varchar(36)   null,
    start_time  bigint   null,
    end_time    bigint null
    );