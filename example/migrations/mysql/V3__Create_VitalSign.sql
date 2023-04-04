create table if not exists VitalSign
(
    id          int auto_increment
    primary key,
    patientId   varchar(36)    null,
    vital_sign_name   varchar(36)    null,
    vital_sign_value  varchar(36) null ,
    vital_sign_unit  varchar(36) null ,
    acq_timestamp bigint   null,
    time_slot bigint null,
    record_timestamp bigint   null,
    userId varchar(36) null

    );

