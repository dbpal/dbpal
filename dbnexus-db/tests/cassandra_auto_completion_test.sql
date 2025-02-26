ALTER ROLE role_test WITH PASSWORD = 'new_password' AND LOGIN = true AND SUPERUSER = false;
20 role_test
ALTER TABLE employees DROP department;
21 employees ; 38 department
ALTER TABLE employees rename  department to department1 ;
21 employees ; 41 department 
ALTER TYPE address ALTER zip_code TYPE text;
19 address ; 29 zip_code
ALTER TYPE address rename zip_code to zip_code;
19 address ; 34 zip_code