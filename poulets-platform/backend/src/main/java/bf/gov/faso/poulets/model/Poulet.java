package bf.gov.faso.poulets.model;

import jakarta.persistence.*;
import java.time.Instant;
import java.util.UUID;

@Entity
@Table(name = "poulets")
public class Poulet {

    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    private UUID id;

    @Version
    private Long version;

    @ManyToOne(fetch = FetchType.LAZY)
    @JoinColumn(name = "eleveur_id", nullable = false)
    private Eleveur eleveur;

    @Column(name = "eleveur_id", insertable = false, updatable = false)
    private UUID eleveurId;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false)
    private Race race;

    @Column(nullable = false)
    private double weight;

    @Column(nullable = false)
    private double price;

    @Column(nullable = false)
    private int quantity = 0;

    private String description;

    @Column(nullable = false)
    private boolean available = true;

    @ManyToOne(fetch = FetchType.LAZY)
    @JoinColumn(name = "categorie_id")
    private Categorie categorie;

    @Column(name = "categorie_id", insertable = false, updatable = false)
    private UUID categorieId;

    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt = Instant.now();

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt = Instant.now();

    @PrePersist
    protected void onCreate() {
        createdAt = Instant.now();
        updatedAt = Instant.now();
    }

    @PreUpdate
    protected void onUpdate() {
        updatedAt = Instant.now();
    }

    // --- Getters and Setters ---

    public UUID getId() { return id; }
    public void setId(UUID id) { this.id = id; }

    public Long getVersion() { return version; }

    public Eleveur getEleveur() { return eleveur; }
    public void setEleveur(Eleveur eleveur) { this.eleveur = eleveur; }

    public UUID getEleveurId() { return eleveurId; }

    public Race getRace() { return race; }
    public void setRace(Race race) { this.race = race; }

    public double getWeight() { return weight; }
    public void setWeight(double weight) { this.weight = weight; }

    public double getPrice() { return price; }
    public void setPrice(double price) { this.price = price; }

    public int getQuantity() { return quantity; }
    public void setQuantity(int quantity) { this.quantity = quantity; }

    public String getDescription() { return description; }
    public void setDescription(String description) { this.description = description; }

    public boolean isAvailable() { return available; }
    public void setAvailable(boolean available) { this.available = available; }

    public Categorie getCategorie() { return categorie; }
    public void setCategorie(Categorie categorie) { this.categorie = categorie; }

    public UUID getCategorieId() { return categorieId; }

    public Instant getCreatedAt() { return createdAt; }
    public Instant getUpdatedAt() { return updatedAt; }
}
