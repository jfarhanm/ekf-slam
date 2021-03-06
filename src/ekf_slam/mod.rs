use na;
use motion_models::base::*;
use crate::normalize_angle;

type IndexType=usize;

//FIXME
pub struct SensorDataType{
    pub r:f32,
    pub phi:f32,
    pub s:f32
}
impl SensorDataType{
    pub fn new(r:f32,phi:f32,s:f32)->SensorDataType{
        SensorDataType{
            r,phi,s
        }
    }
}
type SensorData = SensorDataType;
static mX:(usize,usize)= (0,0);
static mY:(usize,usize)= (1,0);
static mTheta:(usize,usize)= (2,0);

/// This crate contains the implementation of EKFSlam based on the algorithm given in 
/// `table 10.1` of `probabilistic robotics  by thrun and fox`
///
///
/// This library must be used in conjunction with a `motion model` from the `motion_models` crate.
/// `motion_model`s implement the `MotionUpdate2D` trait which contains methods which 
/// calculate new coordinates and the jacobian based on the odometry change information. 
pub struct EKFSlam{
    pub num_landmarks:usize,
    pub feature_size:usize,
    pub mean_matrix:na::DMatrix<f32>,
    pub covariance:na::DMatrix<f32>,
    pub sensor_error_covariance_matrix:na::DMatrix<f32>,
    pub motion_error_covariance_matrix:na::DMatrix<f32>
}


impl EKFSlam{
    /// size of matrix created by this function is (landmarks + 1)*feature_size 
    /// where the 1 is for the motion model of the robot.
    pub fn new(landmarks:usize, feature_size:Option<usize>)->EKFSlam{
        let feature_size_value = feature_size.unwrap_or(3);
        let num_elements = landmarks*feature_size_value + feature_size_value;
        EKFSlam{
            num_landmarks:landmarks,
            feature_size:feature_size_value,
            mean_matrix:na::DMatrix::zeros(num_elements,1),
            covariance:na::DMatrix::zeros(num_elements,num_elements),
            sensor_error_covariance_matrix:na::DMatrix::zeros(feature_size_value,feature_size_value),
            motion_error_covariance_matrix:na::DMatrix::zeros(feature_size_value,feature_size_value)
        }
    }
   

    // FIXME 
    // create a trait for this 
    // use this for motion update
    // fn T:Trait 
    // This API could be made much better 
    pub fn update_odometry_information<T:MotionUpdate2D>(&mut self,left_distance:f32, right_distance:f32, model:&mut T){
        
        let coords= model.update_coords_odometry(left_distance,right_distance);
        //self.motion_update(coords) 
    }


    /// Initial motion update to find $$\overline(bel)$$ . 
    /// The inputs `odometry_l` and `odometry_r` to this function are the change the DISTANCE
    /// travelled by the wheel meaning if you have (theta_l, theta_r) as the angles the wheels have
    /// turned
    /// ``` 
    ///     # let theta_l = 0.5; let theta_r = 0.5; let  R= 0.5;
    ///     let odometry_l = theta_l* R; //where R is the radius of the wheel 
    ///     let odometry_r = theta_r* R; //where r is the radius
    /// ```  
    pub fn motion_update<Mmodel:MotionUpdate2D>(&mut self,model:&mut Mmodel, odometry_l:f32, odometry_r:f32 ){
        let mean_x = self.mean_matrix[mX];
        let mean_y = self.mean_matrix[mY];
        let mean_theta = normalize_angle( self.mean_matrix[mTheta] );
        let initial_state = Model2D::new(mean_x,mean_y,mean_theta);
        let updated_coords = model.update_coords_odometry_stateless(initial_state,odometry_l,odometry_r);
        
        self.mean_matrix[mX] = updated_coords.x;
        self.mean_matrix[mY] = updated_coords.y;
        self.mean_matrix[mTheta] = normalize_angle(updated_coords.theta);
    }
   
    
    /// This gives you the jacobian of the motion model for the extended kalman 
    /// filter, corrected so that the dimensions of the state matrices match.
    ///
    /// `FIND THE VALUE OF THE JACOBIAN BEFORE DOING A MOTION UPDATE!!!`
    ///
    /// Finds $$G_t$$
    pub fn get_dim_corrected_motion_linearizing_matrix<Mmodel:MotionUpdate2D>(&mut self, model:&mut Mmodel, odometry_l:f32, odometry_r:f32)->na::DMatrix<f32>{
        let num_elements = self.num_landmarks*self.feature_size + self.feature_size;        
        let mean_x = self.mean_matrix[mX];
        let mean_y = self.mean_matrix[mY];
        let mean_theta = normalize_angle(self.mean_matrix[mTheta]);
        let pos = Model2D::new(mean_x,mean_y,mean_theta);
        let result_jacobian = model.get_jacobian_stateless(pos, odometry_l,odometry_r);
        // hardcoded for 3x3 features     
       
        //FIXME 
        let mut G_t = na::DMatrix::<f32>::identity(num_elements,num_elements);    
        G_t[(0,2)] = result_jacobian.data[0][2];
        G_t[(1,2)] = result_jacobian.data[1][2];
        
        let l = std::line!();
        Self::print_matrix(format!("[ekf_slam::mod.rs:{} | G_t]",l),&G_t); 
        G_t 
    }

    
    pub fn get_V_M_Vt(&self,model:&mut motion_models::odometry_motion_model::OdometryModel , odom_l:f32,odom_r:f32, radius:f32, alpha1:f32,alpha2:f32,alpha3:f32, alpha4:f32 )->na::DMatrix<f32>{
        let distance_l = odom_l*radius;
        let distance_r = odom_r*radius;
        let t = self.mean_matrix[mTheta];

        let rang_dist = model.update_get_radius_angle_distance(distance_l,distance_r);
        match rang_dist{
            Ok(d) =>{
                let w = d.alpha;
                let s = d.s;
                println!("W is {}",w);
                println!("S is {}",s);


                let mut M = na::DMatrix::<f32>::zeros(2,2);
                M[(0,0)] = alpha2 * (w*w) + alpha1*(s*s);
                M[(1,1)] = alpha4 * (w*w) + alpha3*(s*s);


                let mut V = na::DMatrix::<f32>::zeros(3,2);
                let sint = t.sin();
                let cost = t.cos();
                let sint_wt = (t+w).sin();
                let cost_wt = (t+w).cos();

                V[(0,0)] =  (-sint + sint_wt)/w;
                V[(0,1)] =  s*(sint - sint_wt)/(w*w)  + s*cost_wt/w;
                V[(1,0)] = (cost - cost_wt)/w;
                V[(1,1)] = -(s*(cost - cost_wt))/(w*w) + s*sint_wt/w;
                V[(2,0)] = 0.;
                V[(2,1)] = 1.;
                
                let mut M_Vt = na::DMatrix::<f32>::zeros(2,3); 
                M.mul_to(&V.transpose(),&mut M_Vt);
                let V_M_Vt = V* M_Vt;
                V_M_Vt
            }
            Err(e) =>{
                let s = e.s;
                
                let mut M = na::DMatrix::<f32>::zeros(2,2);
                M[(0,0)] = alpha1*(s*s);
                M[(1,1)] = alpha3*(s*s);

                let mut V = na::DMatrix::<f32>::zeros(3,2);
                                
                let sint = t.sin();
                let cost = t.cos();
                V[(0,0)] =  -sint;
                V[(1,0)] =  cost;
                
                let mut M_Vt = na::DMatrix::<f32>::zeros(2,3); 
                M.mul_to(&V.transpose(),&mut M_Vt);
                let V_M_Vt = V* M_Vt;
                V_M_Vt
            }
        }
    }



    pub fn get_covariance(&mut self)->na::DMatrix<f32>{
        self.covariance.clone()
    }


    
    // Do the corrections here
    // TODO 
    // find out how the error matrix works for this
    // add error matrix values 
    /// Returns $${F_{x}}^{T}R_{T}{F_{x}}$$
    pub fn get_dim_corrected_motion_error_matrix(&mut self)->na::DMatrix<f32>{
        let num_elements = self.num_landmarks*self.feature_size + self.feature_size;
        let mut R_t = na::DMatrix::<f32>::zeros(num_elements,num_elements);    
       
        // TODO this 
        // hardcoded for 3x3 features 
        R_t[(0,0)] = self.motion_error_covariance_matrix[(0,0)];
        R_t[(0,1)] = self.motion_error_covariance_matrix[(0,1)];
        R_t[(0,2)] = self.motion_error_covariance_matrix[(0,2)];
        
        R_t[(1,0)] = self.motion_error_covariance_matrix[(1,0)];
        R_t[(1,1)] = self.motion_error_covariance_matrix[(1,1)];
        R_t[(1,2)] = self.motion_error_covariance_matrix[(1,2)];
        
        R_t[(2,0)] = self.motion_error_covariance_matrix[(2,0)];
        R_t[(2,1)] = self.motion_error_covariance_matrix[(2,1)];
        R_t[(2,2)] = self.motion_error_covariance_matrix[(2,2)];
        
        
        let l = std::line!();
        Self::print_matrix(format!("[ekf_slam::mod.rs:{} | FRF^T]",l),&R_t); 
        

        R_t
    }

    
    // returns sigma_bar
    /// Finds $$\overline{{\Sigma}_t}$$
    pub fn find_linearized_model_covariance_matrix(&mut self, Gt:na::DMatrix<f32>, FTRF:na::DMatrix<f32>)->na::DMatrix<f32>{
    
        let Gt_shape = Gt.shape();
        let mut Gt_S:na::DMatrix<f32> = na::DMatrix::zeros(Gt_shape.0,Gt_shape.1);
        let mut Gt_S_GtT:na::DMatrix<f32> = na::DMatrix::zeros(Gt_shape.0,Gt_shape.1);
        
        
        //let l = std::line!();
        //Self::print_matrix(format!("[ekf_slam::mod.rs:{} | Self.covariance]",l),&self.covariance); 
        
        //zero trail starts her 
        Gt.mul_to(&self.covariance,&mut Gt_S);
        Gt_S.mul_to(&(Gt.transpose()),&mut Gt_S_GtT);
        
        /*
        let l = std::line!();
        Self::print_matrix(format!("[ekf_slam::mod.rs:{} | GEG^T]",l),&Gt_S); 
        */
        
        Gt_S_GtT +FTRF
    }

   
    // FIXME 
    // returns a vector with 3 indices , x;y;z
    // must be changed
    /// This returns the indices (x,y,theta) in the matrix given an index of a landmark
    pub fn get_observation_matrix_index(&self,index:IndexType)->(usize,usize,usize){
        let start = self.feature_size + self.feature_size*index;
        (start,start+1,start+2)
    }


    // TODO
    // FIXME add new element 
    // if landmark never seen before in the algorithm need to mutate a DMatrix for this 
    pub fn add_new_landmark(&mut self, observation:&SensorData){
            let index = self.mean_matrix.shape().0 + 1;
            // push rows to the matrix 
            unimplemented!();
    }



    // TODO 
    // FIXME 
    // IMPLEMENT!!
    // has to check if the element is new 
    // Change this function to 
    /// This function has not been worked on yet but what it is supposed to do 
    /// is to check the level of correlation between the input sensor_data
    /// and all the landmarks in the SLAM matrix to check if the the landmark has
    /// been observed 
    #[warn(unused_features)]
    pub fn is_seen_previously(&self,observation:&SensorData)->Result<(),()>{
        let rows = self.mean_matrix.shape().0;
        Ok(())
    }


    pub fn sensor_update_observation(&mut self, observation:&SensorData, data_index:IndexType, found_indices:&mut Vec<bool>)->(na::DMatrix<f32>,SensorData){
        
        // I have used an ugly hack here
        // with the used_indices
        let mut sensor_mean = na::DMatrix::<f32>::zeros(self.feature_size,1);
       
        let obs_phi  = normalize_angle(observation.phi);
        //FIXME 
        //this thing is an ugly hack 
        // if landmark never seen before thingy 
        // UGLY HACK!!!!!
        if !found_indices[data_index]{
            found_indices[data_index] = true;
            let indices = self.get_observation_matrix_index(data_index);
            self.mean_matrix[indices.0] = self.mean_matrix[mX] + observation.r*(obs_phi + self.mean_matrix[mTheta]).cos();
            self.mean_matrix[indices.1] = self.mean_matrix[mY] + observation.r*(obs_phi + self.mean_matrix[mTheta]).sin();
            self.mean_matrix[indices.2] = 1.0; 
            let l = std::line!();
            println!("[ekf_slam::mod.rs:{}] NEW OBJECT FOUND",l);
        }
        

        // FIXME
        //  The section from here
        //  let data_index = self.uniquely_identify_observation(observation);
        //  This part's fucked !!!
        //  if let Err(()) = self.is_seen_previously(observation){
        //        self.add_new_landmark(observation);       
        //} 
        let indices = self.get_observation_matrix_index(data_index);
        // to here is extremely problematic and soem design decisiond have to be 
        //taken


        let obs_mean_x = self.mean_matrix[(indices.0,0)];
        let obs_mean_y = self.mean_matrix[(indices.1,0)];
        

        let mean_x = self.mean_matrix[mX];
        let mean_y = self.mean_matrix[mY];
        
        let delta_diff_x= obs_mean_x - mean_x;
        let delta_diff_y = obs_mean_y - mean_y;
        
        let q = delta_diff_x.powi(2) + delta_diff_y.powi(2);
        
        
        let zi_bar_r = q.sqrt();
        
        // FIXME 
        let zi_bar_theta = normalize_angle(delta_diff_y.atan2(delta_diff_x) - self.mean_matrix[mTheta]);
        
        //let zi_bar_theta = (delta_diff_y/delta_diff_x).atan();

        let signature_mean = 1.0; // FIXME IDK WHAT TO DO THERE
        
        
        let l = std::line!();
        println!("[ekf_slam::mod.rs:{}] zi_diff_x:{} | zi_diff_y:{} | z_diff_theta:{} ",l,delta_diff_x,delta_diff_y,zi_bar_theta);



        /*
        //FIXME 
        let l = std::line!();
        println!("[ekf_slam::mod.rs:{}] zi_bar_r:{} | zi_bar_theta:{} | signature_mean:{}",l,zi_bar_r,zi_bar_theta,signature_mean);
        */

        let q_delta_x =q*delta_diff_x;
        let q_delta_y =q*delta_diff_y;
        let q_root = q.sqrt();
        
        let num_features = self.feature_size;
        let num_elements = self.feature_size*self.num_landmarks + self.feature_size;
        
        // {H_t}^{i}
        let mut H_matrix = na::DMatrix::<f32>::zeros(num_features,num_elements);
       
        // Line 16 of the EKF algorithm in thrun 
        // Everything after this is hardcoded for 3 features  
        //big error here 


        // ROW 1 
        H_matrix[(0,0)] = -delta_diff_x/zi_bar_r;   
        H_matrix[(0,1)] = -delta_diff_y/zi_bar_r;   
        H_matrix[(0,2)] = 0.; 
        
        H_matrix[(0,indices.0)] = delta_diff_x/zi_bar_r;
        H_matrix[(0,indices.1)] = delta_diff_y/zi_bar_r;
        H_matrix[(0,indices.2)] = 0.;


        // ROW 2
        H_matrix[(1,0)] = delta_diff_y/q;
        H_matrix[(1,1)] = -delta_diff_y/q;
        H_matrix[(1,2)] = -1.;
        
        H_matrix[(1,indices.0)] = -delta_diff_y/q;
        H_matrix[(1,indices.1)] = delta_diff_x/q;
        H_matrix[(1,indices.2)] = 0.;

        // ROW 3
        H_matrix[(2,0)] = 0.;
        H_matrix[(2,1)] =0.; 
        H_matrix[(2,2)] = 0.;
        
        H_matrix[(2,indices.0)] = 0.;
        H_matrix[(2,indices.1)] = 0.;
        H_matrix[(2,indices.2)] = 1.;
        
        (H_matrix ,SensorDataType::new(zi_bar_r,zi_bar_theta,signature_mean))
    }
    
   

    // FIXME 
    // There may be errors here 
    pub fn get_kalman_gain(&mut self, Sigma_bar:&mut na::DMatrix<f32>,H_matrix:&mut na::DMatrix<f32>)->Result<na::DMatrix<f32>,&'static str>{
   
        let Sigma_bar_shape = Sigma_bar.shape();
        let H_matrix_shape = H_matrix.shape();
       
        //FIXME 
        let H_matrix_transpose_shape = (H_matrix_shape.1,H_matrix_shape.0);
        let mut S_Ht = na::DMatrix::<f32>::zeros(Sigma_bar_shape.0,H_matrix_transpose_shape.1);
        let mut H_S_Ht = na::DMatrix::<f32>::zeros(H_matrix_shape.0,H_matrix_shape.0);    
        Sigma_bar.mul_to(&H_matrix.transpose(),&mut S_Ht);
        
        
        /*
        let l = std::line!();
        Self::print_matrix(format!("[ekf_slam::mod.rs:{} | Sigma_bar]",l),&Sigma_bar); 
        
        let l = std::line!();
        Self::print_matrix(format!("[ekf_slam::mod.rs:{} | H_matrix]",l),&H_matrix); 
        */
        

        H_matrix.mul_to(&S_Ht,&mut H_S_Ht);
        let H_S_Ht_shape = H_S_Ht.shape();
        let mut H_S_HtpQ = na::DMatrix::<f32>::zeros(H_S_Ht_shape.0,H_S_Ht_shape.1);
        

        H_S_Ht.add_to(&self.sensor_error_covariance_matrix,&mut H_S_HtpQ);
    
       //FIXME
       //Self::print_matrix(String::from("H_S_HtpQ"),&H_S_HtpQ);
       let H_S_HtpQinv  = match H_S_HtpQ.try_inverse(){
            Some(a)=>{a}
            None=>{
                return Err("Cannot find the inverse")
            }
       };
        
       let H_S_HtpQinv_shape = H_S_HtpQinv.shape();
      
        

       // Multipy by S_Ht 
       // error here ?
       //
       //can panic!
       let mut Ht_H_S_HtpQinv = na::DMatrix::<f32>::zeros(H_matrix_transpose_shape.0,H_S_HtpQinv_shape.1);
       
       
       H_matrix.transpose().mul_to(&H_S_HtpQinv,&mut Ht_H_S_HtpQinv);
       
       let mut Kalman_gain = na::DMatrix::<f32>::zeros(Sigma_bar_shape.0,Ht_H_S_HtpQinv.shape().1);
       
       Sigma_bar.mul_to(&Ht_H_S_HtpQinv,&mut Kalman_gain);
       
       Ok(Kalman_gain)

    }
    



    // FIXME 
    // This definitely contains errors 
    // Look at the iter error matrix 
    /// Does $$\overline{{\mu}_t} = \overline{{\mu}_t} + {K_i}^{i}( {z_t}^i -
    /// \overline{{z_t}^{i}} )$$
    pub fn apply_mean_kalman_gain(&mut self, 
                                  kalman_gain:&na::DMatrix<f32>,
                                  observation_estimated:&SensorData,
                                  observation_sensor:&SensorData){
        
        let mut z_diff = na::DMatrix::<f32>::zeros(self.feature_size,self.feature_size);
        z_diff[(0,0)] = observation_sensor.r - observation_estimated.r; 
        z_diff[(1,0)] = observation_sensor.phi - observation_estimated.phi; 
        z_diff[(2,0)] = observation_sensor.s - observation_estimated.s; 
        
        
        let l = std::line!();
        println!("[ekf_slam::mod.rs:{}] z_diff[R]:{} | z_diff[phi]:{} | z_diff[s]:{} ",l,z_diff[(0,0)] , z_diff[(1,0)] , z_diff[(2,0)]  );
        
    
        
        let l = std::line!();
        Self::print_matrix( format!("[ekf_slam::mod.rs:{} Kalman gain]",l),&kalman_gain) ;       
        


        let mut K_mul_z_diff = na::DMatrix::<f32>::zeros(kalman_gain.shape().0,z_diff.shape().1);
        kalman_gain.mul_to(&z_diff,&mut K_mul_z_diff);
    
        //THIS DOES NOT WORK 
        //FIXME this might shit the bed 
        self.mean_matrix.iter_mut().zip(K_mul_z_diff.iter_mut()).for_each(|(m,n)|{
            *m = *m+*n; 
        });
    }
    



    // FIXME 
    // this might need fixing 
    // everything is  a fixme now 
    /// final step 
    /// line 19 of algorithm 
    /// $$\overline{{\Sigma}_t} = (I - {K_t}^{i}{H_t}^{i}){\overline{{\Sigma}_t}}$$
    /// Returns new covariance matrix 
    pub fn get_covariance_kalman_gain(&mut self,
                                        kalman_gain:na::DMatrix<f32>,
                                        H_matrix:na::DMatrix<f32>,
                                        covariance:na::DMatrix<f32>)->na::DMatrix<f32>{
        //possible error here?
        let K_H = kalman_gain*H_matrix;
        let mut I = na::DMatrix::<f32>::identity(K_H.shape().0,K_H.shape().1);
        I = I - K_H;
        I*covariance
    }


    // FIXME 
    // create a valid trait for observatiob and a valid trait for return type 
    // preferably something like 
    // Observation{
    //      /*fields*/
    // } 
    // IndexType{
    //  /*fields*/
    // }
    // Actually this should be turned into a trait implementation? 
    /// This requires some work from us
    /// The function here is $${C_t}^{i}
    pub fn uniquely_identify_observation(&mut self, observation:&SensorData)->IndexType{
        unimplemented!()
    }
    

    /// Set covariance matrix 
    pub fn set_covariance_matrix(&mut self,covariance:na::DMatrix<f32>){
        self.covariance = covariance;
    }



    /// Sets the inital sensor error covariance matrix 
    pub fn set_sensor_error_covariance_matrix(&mut self, mat:na::DMatrix<f32>){
        if self.sensor_error_covariance_matrix.shape()!=mat.shape(){
            panic!("MATRIX SIZES DO NOT MATCH")
        }
        self.sensor_error_covariance_matrix = mat;
    }


    pub fn set_motion_error_covariance_matrix(&mut self, mat:na::DMatrix<f32>){
        if self.motion_error_covariance_matrix.shape()!=mat.shape(){
            panic!("MATRIX SIZES DO NOT MATCH")
        }
        self.motion_error_covariance_matrix = mat;
    }

    
    //model noise matrix
    //FIXME DISPLAY 
    pub fn display_mean_matrix(&self){
        Self::print_matrix( String::from("MEAN MATRIX"),  &self.mean_matrix);
    }

    // DEBUG
    pub fn print_matrix(title:String,mat:&na::DMatrix<f32>){
        println!("[DMatrix]-----------------------------[{}]",title);
        mat.row_iter().for_each(|m|{
            m.iter().for_each(|n|{
                print!(" {} ",n);
            });
            println!();
        });
        println!("\n\n");
    } 

    //TODO 
    // add get x , y trait 

}






#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    #[test]
    fn slam_test() {
        /*
        let base_length = 20.0;
        let wheel_radius = 10.0;
        let odom_l = 20.0;
        let odom_r = 21.1;
        
        let test_observation= super::SensorDataType{
            r:20.0,
            phi:30.0,
            s:40.0
        }; 


        let mut slam = super::EKFSlam::new(10,None);
        let features= slam.feature_size;
        let mut sensor_error_covariance_matrix= na::DMatrix::<f32>::zeros(features, features);
        sensor_error_covariance_matrix[(0,0)] = 20.0;   
        sensor_error_covariance_matrix[(1,1)] = 10.0;   
        sensor_error_covariance_matrix[(2,2)] = 5.0;   
        slam.set_sensor_error_covariance_matrix(sensor_error_covariance_matrix);


    
        let mut motion_error_covariance_matrix= na::DMatrix::<f32>::zeros(features, features);
        motion_error_covariance_matrix[(0,0)] = 20.0;   
        motion_error_covariance_matrix[(1,1)] = 10.0;   
        motion_error_covariance_matrix[(2,2)] = 5.0;   
        slam.set_motion_error_covariance_matrix(motion_error_covariance_matrix);





        let mut motion_model = motion_models::odometry_motion_model::OdometryModel::new(base_length);
        
        let g_t = slam.get_dim_corrected_motion_linearizing_matrix(&mut motion_model,odom_l*wheel_radius,odom_r*wheel_radius);
        slam.motion_update(&mut motion_model,odom_l,odom_r);
        
        let r_t = slam.get_dim_corrected_motion_error_matrix();
        let mut sigma_bar = slam.find_linearized_model_covariance_matrix(g_t,r_t);
        let (mut h_matrix, mut estimated_observation) = slam.sensor_update_observation(&test_observation,9);
        
        let mut kalman_gain = match slam.get_kalman_gain(&mut sigma_bar,&mut h_matrix){
            Ok(g)=>g,
            Err(e)=>{panic!(e)}
        };

        slam.apply_mean_kalman_gain(&kalman_gain,
                                  &estimated_observation
                                  ,&test_observation);
        //slam.motion_update();
        slam.display_mean_matrix();
        slam.apply_covariance_kalman_gain(
                                        kalman_gain,
                                        h_matrix,
                                        sigma_bar);       
    
    */
        }
}
