mod common;
use common::*;

#[tokio::test]
async fn publishes_read_hold_mqtt() {
    common_setup();

    // setup config with only mqtt enabled
    let mut config = Factory::example_config();
    config.influx.enabled = false;
    config.databases[0].enabled = false;
    let config = Rc::new(config);

    let inverter = &config.inverters[0];

    let channels = Channels::new();

    let coordinator = Coordinator::new(Rc::clone(&config), channels.clone());

    let tf = async {
        let mut to_influx = channels.to_influx.subscribe();
        let mut to_mqtt = channels.to_mqtt.subscribe();
        let mut to_db = channels.to_database.subscribe();

        // simulate ReadHold in from inverter
        let packet = Packet::TranslatedData(lxp::packet::TranslatedData {
            datalog: inverter.datalog,
            device_function: lxp::packet::DeviceFunction::ReadHold,
            inverter: inverter.serial,
            register: 12,
            values: vec![22, 6],
        });
        channels
            .from_inverter
            .send(lxp::inverter::ChannelData::Packet(packet.clone()))?;

        // verify MQTT output
        assert_eq!(
            to_mqtt.recv().await?,
            mqtt::ChannelData::Message(mqtt::Message {
                topic: format!("{}/hold/12", inverter.datalog),
                payload: "1558".to_owned()
            })
        );
        // verify nothing sent to influx or database
        assert_eq!(to_influx.try_recv(), Err(TryRecvError::Empty));
        assert_eq!(to_db.try_recv(), Err(TryRecvError::Empty));

        coordinator.stop();

        Ok::<(), anyhow::Error>(())
    };

    futures::try_join!(coordinator.start(), tf).unwrap();
}

#[tokio::test]
async fn handles_read_input_all() {
    common_setup();

    let mut config = Factory::example_config();
    config.influx.enabled = true;
    config.databases[0].enabled = true;
    let config = Rc::new(config);
    let inverter = &config.inverters[0];

    let channels = Channels::new();

    let coordinator = Coordinator::new(Rc::clone(&config), channels.clone());

    let tf = async {
        let mut to_influx = channels.to_influx.subscribe();
        let mut to_mqtt = channels.to_mqtt.subscribe();
        let mut to_database = channels.to_database.subscribe();

        // simulate ReadHold in from inverter
        let packet = Packet::TranslatedData(lxp::packet::TranslatedData {
            datalog: inverter.datalog,
            device_function: lxp::packet::DeviceFunction::ReadInput,
            inverter: inverter.serial,
            register: 0,
            values: vec![1; 254],
        });
        channels
            .from_inverter
            .send(lxp::inverter::ChannelData::Packet(packet.clone()))?;

        // verify MQTT output. numbers look odd but they're just cos of all the 1's in the input
        assert_eq!(
            to_mqtt.recv().await?,
            mqtt::ChannelData::Message(mqtt::Message {
                topic: format!("{}/inputs/all", inverter.datalog),
                payload: "{\"status\":257,\"v_pv\":77.1,\"v_pv_1\":25.7,\"v_pv_2\":25.7,\"v_pv_3\":25.7,\"v_bat\":25.7,\"soc\":1,\"soh\":1,\"p_pv\":771,\"p_pv_1\":257,\"p_pv_2\":257,\"p_pv_3\":257,\"p_charge\":257,\"p_discharge\":257,\"v_ac_r\":25.7,\"v_ac_s\":25.7,\"v_ac_t\":25.7,\"f_ac\":2.57,\"p_inv\":257,\"p_rec\":257,\"pf\":0.257,\"v_eps_r\":25.7,\"v_eps_s\":25.7,\"v_eps_t\":25.7,\"f_eps\":2.57,\"p_eps\":257,\"s_eps\":257,\"p_to_grid\":257,\"p_to_user\":257,\"e_pv_day\":77.1,\"e_pv_day_1\":25.7,\"e_pv_day_2\":25.7,\"e_pv_day_3\":25.7,\"e_inv_day\":25.7,\"e_rec_day\":25.7,\"e_chg_day\":25.7,\"e_dischg_day\":25.7,\"e_eps_day\":25.7,\"e_to_grid_day\":25.7,\"e_to_user_day\":25.7,\"v_bus_1\":25.7,\"v_bus_2\":25.7,\"e_pv_all\":5052902.699999999,\"e_pv_all_1\":1684300.9,\"e_pv_all_2\":1684300.9,\"e_pv_all_3\":1684300.9,\"e_inv_all\":1684300.9,\"e_rec_all\":1684300.9,\"e_chg_all\":1684300.9,\"e_dischg_all\":1684300.9,\"e_eps_all\":1684300.9,\"e_to_grid_all\":1684300.9,\"e_to_user_all\":1684300.9,\"t_inner\":257,\"t_rad_1\":257,\"t_rad_2\":257,\"t_bat\":257,\"runtime\":16843009,\"max_chg_curr\":2.57,\"max_dischg_curr\":2.57,\"charge_volt_ref\":25.7,\"dischg_cut_volt\":25.7,\"bat_status_0\":257,\"bat_status_1\":257,\"bat_status_2\":257,\"bat_status_3\":257,\"bat_status_4\":257,\"bat_status_5\":257,\"bat_status_6\":257,\"bat_status_7\":257,\"bat_status_8\":257,\"bat_status_9\":257,\"bat_status_inv\":257,\"bat_count\":257,\"bat_capacity\":257,\"bat_current\":2.57,\"bms_event_1\":257,\"bms_event_2\":257,\"max_cell_voltage\":2.57,\"min_cell_voltage\":2.57,\"max_cell_temp\":2.57,\"min_cell_temp\":2.57,\"bms_fw_update_state\":257,\"cycle_count\":257,\"vbat_inv\":25.7,\"time\":1646370367,\"datalog\":\"2222222222\"}".to_owned()
            })
        );

        // verify influx and database output
        let d = unwrap_influx_channeldata_input_data(to_influx.recv().await?);
        assert_eq!(d["soc"], 1);
        assert_eq!(d["v_pv"], 77.1);
        let d = unwrap_database_channeldata_read_input_all(to_database.recv().await?);
        assert_eq!(d.soc, 1);
        assert_eq!(d.v_pv, 77.1);

        coordinator.stop();

        Ok::<(), anyhow::Error>(())
    };

    futures::try_join!(coordinator.start(), tf).unwrap();
}

#[tokio::test]
async fn complete_path_read_hold_command() {
    common_setup();

    let config = Rc::new(Factory::example_config());

    let inverter = &config.inverters[0];

    let channels = Channels::new();

    let coordinator = Coordinator::new(Rc::clone(&config), channels.clone());

    let tf = async {
        let mut to_inverter = channels.to_inverter.subscribe();
        let mut to_mqtt = channels.to_mqtt.subscribe();

        // simulate:
        //   mqtt incoming "read this hold" command
        let message = mqtt::Message {
            topic: "cmd/all/read/hold/12".to_owned(),
            payload: "".to_owned(),
        };
        channels
            .from_mqtt
            .send(mqtt::ChannelData::Message(message))
            .unwrap();

        //   wait for inverter to receive the right packet
        let packet = Packet::TranslatedData(lxp::packet::TranslatedData {
            datalog: inverter.datalog,
            device_function: lxp::packet::DeviceFunction::ReadHold,
            inverter: inverter.serial,
            register: 12,
            values: vec![1, 0],
        });
        assert_eq!(
            to_inverter.recv().await?,
            lxp::inverter::ChannelData::Packet(packet),
        );

        //   send the packet back from the inverter
        let reply = Packet::TranslatedData(lxp::packet::TranslatedData {
            datalog: inverter.datalog,
            device_function: lxp::packet::DeviceFunction::ReadHold,
            inverter: inverter.serial,
            register: 12,
            values: vec![22, 6],
        });
        channels
            .from_inverter
            .send(lxp::inverter::ChannelData::Packet(reply))
            .unwrap();

        //   wait for mqtt to get the right responses
        assert_eq!(
            to_mqtt.recv().await?,
            mqtt::ChannelData::Message(mqtt::Message {
                topic: "2222222222/hold/12".to_owned(),
                payload: "1558".to_owned()
            })
        );
        assert_eq!(
            to_mqtt.recv().await?,
            mqtt::ChannelData::Message(mqtt::Message {
                topic: "result/2222222222/read/hold/12".to_owned(),
                payload: "OK".to_owned()
            })
        );

        coordinator.stop();

        Ok::<(), anyhow::Error>(())
    };

    futures::try_join!(coordinator.start(), tf).unwrap();
}
