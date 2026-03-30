CREATE INDEX DeviceData_id_device_no_msh_time_index
    ON DeviceData (id, device_no, msh_time);
CREATE INDEX DeviceData__patientid
    ON DeviceData (patientId);

CREATE INDEX VitalSign_patientId_id_acq_timestamp_time_slot_index
    ON VitalSign (patientId, id, acq_timestamp, time_slot);


