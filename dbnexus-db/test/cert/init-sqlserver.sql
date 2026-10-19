
CREATE DATABASE testdb;
GO
USE testdb;
GO
CREATE LOGIN testuser WITH PASSWORD = 'testpass';
GO
CREATE USER testuser FOR LOGIN testuser;
GO
ALTER ROLE db_owner ADD MEMBER testuser;
GO
