create table if not exists DeviceData
(
    id          int auto_increment
        primary key,
    device_no   varchar(36)    null,

    patientId   varchar(36)    null,
    patientName varchar(36)    null ,
    hisId                    varchar(36)   null,
    departmentId             varchar(255)  null,
    departmentName           varchar(128)  null,
    SickbedNo   varchar(63) null ,

    msh_time    bigint   null,
    msh_type    text     null,
    vital_signs longtext null,
    hl7         longtext null
);

