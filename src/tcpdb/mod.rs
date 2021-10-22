
extern crate redis;

pub struct TcpDatabase{
    connection: redis::Connection
}

impl TcpDatabase {
    pub fn new() -> Self {
        let client = redis::Client::open("redis://127.0.0.1:6379").expect("Could not connect to redis!");
        let connection = client.get_connection().expect("Could not connect to redis!");
        Self {
            connection,
        }
    }

    pub fn increment_z_to_a_syn_counter(&mut self, flow: &String) -> Option<i32> {
        if let Some(counter) = redis::cmd("HINCRBY")
            .arg(&flow)
            .arg("z_to_a_syn_counter")
            .arg(1)
            .query(&mut self.connection)
            .unwrap() { 
                return Some(counter) 
            }
        None
    }
    
    pub fn get_z_to_a_syn_counter(&mut self, flow: &String) -> Option<i32> {
        if let Some(counter) = redis::cmd("HGET")
            .arg(&flow)
            .arg("z_to_a_syn_counter")
            .query(&mut self.connection)
            .unwrap() { 
                return Some(counter) 
            }
        None
    }
    
    pub fn get_redis_keys(&mut self) -> Option<Vec<String>> {
        if let Some(keys) = redis::cmd("KEYS")
            .arg("*")
            .query(&mut self.connection)
            .expect("Could not get redis keys") { 
                return Some(keys) 
            }
        None
    }
    
    pub fn add_tcp_connection(&mut self, flow: &String, a_ip: &String, z_ip: &String) -> bool {
        let result: redis::RedisResult<String> = redis::cmd("HGET").arg(&flow).arg("a_ip").query(&mut self.connection);
        match result {
    
            // Connection already exists.
            Ok(_) => { 
                return false; 
            }
    
            // Create the connection.
            Err(_) => {
                let _: () = redis::cmd("HSET").arg(&flow).arg("a_to_z_syn_counter").arg(0).query(&mut self.connection).unwrap();
                let _: () = redis::cmd("HSET").arg(&flow).arg("z_to_a_syn_counter").arg(0).query(&mut self.connection).unwrap();
                let _: () = redis::cmd("HSET").arg(&flow).arg("a_ip").arg(a_ip).query(&mut self.connection).unwrap();
                let _: () = redis::cmd("HSET").arg(&flow).arg("z_ip").arg(z_ip).query(&mut self.connection).unwrap();
                return true
            }
        }
    }
}