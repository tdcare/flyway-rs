create index DeviceData_id_device_no_msh_time_index
    on tdbox_service.DeviceData (id, device_no, msh_time);
create index DeviceData__patientid
    on tdbox_service.DeviceData (patientId);

create index VitalSign_patientId_id_acq_timestamp_time_slot_index
    on tdbox_service.VitalSign (patientId, id, acq_timestamp, time_slot);


