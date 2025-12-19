use crate::models::{ user::User};
use actix_web::{HttpResponse, Responder, Result, web};
use chrono::Utc;
use uuid::Uuid;

// pub async fn login_handler(login_data: web::Json<LoginSchema>) -> impl Responder {
//     let user = login_data.into_inner();
//     println!("Username: {}", user.username);
//     println!("Email: {}", user.email);

//     HttpResponse::Ok().json("Login successful")
// }



// pub async fn create_user(user_data: web::Json<LoginSchema>) -> Result<HttpResponse> {
//     let login_schema = user_data.into_inner();

//     let new_user = User {
//         id: Uuid::new_v4(),
//         username: login_schema.username,
//         email: login_schema.email,
//         password: login_schema.password,
//         created_at: Utc::now(),
//         updated_at: Utc::now(),
//     };

//     Ok(HttpResponse::Created().json(new_user))
// }
