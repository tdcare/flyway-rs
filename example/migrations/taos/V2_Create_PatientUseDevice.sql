create table if not exists PatientUseDevice
(
    ts timestamp,
    id          int,
    device_no    nchar(36)      ,
    patientId    nchar(36)      ,
    hisId        nchar(36)     ,
    start_time  bigint     ,
    end_time    bigint   
    );