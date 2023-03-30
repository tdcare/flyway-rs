create table if not exists DeviceData
(
   ts timestamp,
    id          int ,
    device_no   nchar(36),

    patientId   nchar(36)     ,
    patientName nchar(36)      ,
    hisId                    nchar(36)    ,
    departmentId             nchar(255)   ,
    departmentName           nchar(128)   ,
    SickbedNo   nchar(63)   ,

    msh_time    bigint    ,
    msh_type    nchar(36)      ,
    vital_signs nchar(2048)  ,
    hl7         nchar(2048)  
);

