use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use clove1db::{
    dto::{InputDto, OutputDto},
    entity::Entity,
    storage::{DatabaseConfig, Storage, StorageConfig},
    units::{ClError, Result},
};

// ═══════════════════════════════════════════════════════════
// SCENARIO 1: Security & Hiding Data (User Account)
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserAccount {
    id: String,
    username: String,
    password_hash: String,
    created_at: i64,
}

impl Entity for UserAccount {
    fn entity_id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Deserialize)]
struct RegisterUserDto {
    username: String,
    raw_password: String,
}

impl InputDto<UserAccount> for RegisterUserDto {
    fn validate(&self) -> Result<()> {
        if self.raw_password.len() < 6 {
            return Err(ClError::Validation("Password must be at least 6 chars".into()).into());
        }
        Ok(())
    }

    fn into_entity(self) -> Result<UserAccount> {
        // Business Logic: هنا نقوم بتشفير الباسورد قبل تحويله لـ Entity
        let simulated_hash = format!("hashed_{}", self.raw_password);

        Ok(UserAccount {
            id: uuid::Uuid::new_v4().to_string(),
            username: self.username,
            password_hash: simulated_hash,
            created_at: chrono::Utc::now().timestamp(),
        })
    }
}

// Output DTO: لا يحتوي على الـ password_hash نهائياً (آمن للـ API)
#[derive(Debug, Serialize)]
struct UserPublicProfile {
    id: String,
    username: String,
}

impl OutputDto<UserAccount> for UserPublicProfile {
    fn from_entity(e: UserAccount) -> Self {
        Self {
            id: e.id,
            username: e.username,
        }
    }
}

// ═══════════════════════════════════════════════════════════
// SCENARIO 2: Business Logic & Calculations (E-commerce Order)
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Order {
    id: String,
    item_name: String,
    base_price: f64,
    tax: f64,
    total_amount: f64,
}

impl Entity for Order {
    fn entity_id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Deserialize)]
struct PlaceOrderDto {
    item_name: String,
    base_price: f64,
}

impl InputDto<Order> for PlaceOrderDto {
    fn validate(&self) -> Result<()> {
        if self.base_price <= 0.0 {
            return Err(ClError::Validation("Price must be > 0".into()).into());
        }
        Ok(())
    }

    fn into_entity(self) -> Result<Order> {
        // Business Logic:
        let tax = self.base_price * 0.15; // 15% Tax
        let total_amount = self.base_price + tax;

        Ok(Order {
            id: uuid::Uuid::new_v4().to_string(),
            item_name: self.item_name,
            base_price: self.base_price,
            tax,
            total_amount,
        })
    }
}

// Output DTO: إرجاع فاتورة منسقة
#[derive(Debug, Serialize)]
struct OrderReceipt {
    order_id: String,
    summary: String,
    final_total: f64,
}

impl OutputDto<Order> for OrderReceipt {
    fn from_entity(e: Order) -> Self {
        Self {
            order_id: e.id,
            summary: format!("{} (+15% Tax)", e.item_name),
            final_total: e.total_amount,
        }
    }
}

// ═══════════════════════════════════════════════════════════
// SCENARIO 3: Custom ID & Upsert (System Settings)
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SystemSetting {
    key: String, // هنا نستخدم الـ Key كـ ID بدلاً من UUID
    value: String,
}

impl Entity for SystemSetting {
    fn entity_id(&self) -> &str {
        &self.key
    }
}

#[derive(Debug, Deserialize)]
struct UpdateSettingDto {
    key: String,
    value: String,
}

impl InputDto<SystemSetting> for UpdateSettingDto {
    fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn into_entity(self) -> Result<SystemSetting> {
        Ok(SystemSetting {
            key: self.key, // ID مخصص
            value: self.value,
        })
    }
}

// Output DTO: بسيط جداً يعكس الـ Entity تماماً
#[derive(Debug, Serialize)]
struct SettingResponse {
    key: String,
    value: String,
}

impl OutputDto<SystemSetting> for SettingResponse {
    fn from_entity(e: SystemSetting) -> Self {
        Self {
            key: e.key,
            value: e.value,
        }
    }
}

// ═══════════════════════════════════════════════════════════
// MAIN EXECUTION
// ═══════════════════════════════════════════════════════════

fn main() -> Result<()> {
    let base_dir = PathBuf::from("./examples_data/05_domain_dto_patterns");

    let storage = Storage::builder(StorageConfig::default())
        .add_database(
            DatabaseConfig::new("app_db", "app")
                .dir_path(base_dir)
                .register::<UserAccount>("users")
                .register::<Order>("orders")
                .register::<SystemSetting>("settings"),
        )
        .build()?;

    // ── 1. Scenario: Security (User Profile) ────────────────
    println!("\n━━━ Scenario 1: User Security ━━━");
    let user_domain = storage.domain::<UserAccount>();

    let new_user = user_domain.create::<RegisterUserDto, UserPublicProfile>(RegisterUserDto {
        username: "admin_root".into(),
        raw_password: "supersecret123".into(),
    })?;

    // لاحظ أن المخرجات `UserPublicProfile` لا تحتوي على `password_hash` نهائياً
    println!("  ✅ Created User (Public View): {:?}", new_user);
    // println!("Hash: {}", new_user.password_hash); // ❌ هذا سيعطي خطأ برمجي (Compile Error) وهو المطلوب أمنياً

    // ── 2. Scenario: Business Logic (Orders) ────────────────
    println!("\n━━━ Scenario 2: Order Calculations ━━━");
    let order_domain = storage.domain::<Order>();

    let receipt = order_domain.create::<PlaceOrderDto, OrderReceipt>(PlaceOrderDto {
        item_name: "Gaming Monitor".into(),
        base_price: 1000.0,
    })?;

    // الـ DTO قام بحساب الضريبة (150) والإجمالي (1150) وتنسيق الملخص
    println!("  ✅ Order Receipt Generated: {:?}", receipt);

    // ── 3. Scenario: Custom ID (Settings) ───────────────────
    println!("\n━━━ Scenario 3: Custom ID Settings ━━━");
    let settings_domain = storage.domain::<SystemSetting>();

    // نستخدم الـ Key كمعرّف (ID) مباشر
    let setting =
        settings_domain.create::<UpdateSettingDto, SettingResponse>(UpdateSettingDto {
            key: "THEME_COLOR".into(),
            value: "DARK_MODE".into(),
        })?;
    println!("  ✅ Setting Saved: {:?}", setting);

    // تحديث الإعداد باستخدام نفس الـ Key
    let updated_setting = settings_domain.update::<UpdateSettingDto, SettingResponse>(
        "THEME_COLOR", // الـ ID هو الكلمة نفسها
        UpdateSettingDto {
            key: "THEME_COLOR".into(),
            value: "LIGHT_MODE".into(),
        },
    )?;
    println!("  ✅ Setting Updated: {:?}", updated_setting);

    println!("\n✅ Domain & DTO Patterns Example completed successfully!");
    Ok(())
}
